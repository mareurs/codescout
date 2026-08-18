# Guide Ledger Phase B — Session Identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the guide ledger a conversation identity it can trust, so `/clear` stops corrupting it and clients without a session id stop starving.

**Architecture:** Replace the current three-link key chain (env var → per-project file → random UUID) with the spec's two-tier model: a *keyed* tier when a conversation identity is obtainable, and an *anonymous* tier bounded by an idle TTL when none is. Add a pid-keyed rendezvous so the companion plugin can push a fresh session id into an already-running server — the only way to observe `/clear`, which mints a new conversation id without respawning the MCP subprocess.

**Tech Stack:** Rust (rmcp 1.3, serde_json, chrono, parking_lot, libc); Node ESM for the companion hook; bash for the hook test harness.

**Spec:** `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` — §1 key chain, §6 companion rendezvous, §7 TTL policy. Read §§1, 6, 7 and the "Suggested phasing" table before starting.

**Predecessor:** Phase A (`docs/superpowers/plans/2026-08-18-guide-ledger-phase-a-storage.md`) shipped storage, the stamped on-disk shape, `re_arm`/`expire_idle`, and the 35-day GC. Its mechanisms exist; Phase B is where two of them get their first production caller.

**Successor:** Phase C consumes `Rendezvous::is_active()` (Task 5) to gate its re-arm predicate. Do **not** implement spec §4 or §5 here.

## Global Constraints

- **Degrade to re-sending, never to suppressing.** Every failure path — missing file, unparseable JSON, absent hook, unknown pid — must end with a guide being re-sent, not withheld. This is the spec's governing invariant and it outranks token efficiency in every trade.
- **Agent-Agnostic Design.** Nothing may branch on `clientInfo.name` or assume Claude Code. The server must be fully correct with no companion plugin installed.
- **Tests must never call `std::env::set_var`.** Mutating `environ` while other test threads call `getenv` is UB. Inject through `ServerEnv` (`src/server.rs:55-67`) instead. Hard gate, not a style note.
- **No test may read, write, or GC the developer's real per-user state directory.** Inject `guide_hints_dir` and `servers_dir`; for spawned-binary tests override `XDG_STATE_HOME` on the child.
- **`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` must all pass before any task is reported complete.**
- **Commit with a pathspec** (`git commit -m "..." -- <paths>`). A concurrent session shares this checkout; a bare `git commit -a` sweeps its work. See F-54 in `docs/trackers/bug-fix-session-log.md`.
- **Two repos.** Tasks 1–5 and 7 land in `codescout`. Task 6 lands in `/home/marius/work/claude/claude-plugins/codescout-companion` — a *separate git repo* with its own commit. Never commit one repo's changes from the other.

## Rulings made before execution

These settle questions the spec leaves open. Recorded here so an implementer does not re-litigate them mid-task.

1. **`_meta` rank 3 is deferred, not dropped.** Spec "Open parameters" #3 leaves it open. Its unique value is tracking a conversation change *without* a rendezvous — and this phase ships the rendezvous, so that value is covered. ~30 lines with no current sender is scope we do not need. *Cost if wrong:* when a client eventually sends `_meta`, we add the rank then; nothing built here blocks it.
2. **`started_at` is the server's own wall-clock construction time, not the OS process start time.** The spec names the field without saying which. OS process start time needs per-platform code (`/proc/<pid>/stat` field 22, `sysctl` on macOS, `GetProcessTimes` on Windows) for no benefit here: the server rewrites its own entry at construction, so a recycled pid's stale entry is overwritten by its rightful owner. *Cost if wrong:* a recycled pid could briefly carry a stale session id in a file no live server reads — benign, and collected by the liveness GC.
3. **`cc_session_id` splits into two values with different requirements.** The ledger key must not collide between concurrent windows; `usage.db` correlation tolerates collision and is per-project anyway. Task 1 gives the ledger the new `SessionKey` and leaves `UsageRecorder` (`src/server.rs:787-791`) reading the old chain unchanged, including the `.codescout/cc_session_id` file fallback. *Cost if wrong:* none identified — it is strictly less behaviour change than folding them together.
4. **A fully-expired ledger re-fires the session opener, and that is now a decision.** Confirmed by the user 2026-08-18. Task 3 documents and pins it rather than preventing it. *Cost if wrong:* an anonymous-tier client idle past its TTL receives the orientation guide again — the correct direction under the governing invariant.

## File Structure

**codescout — created:**

- `src/tools/session_key.rs` — pure conversation-identity resolution. One responsibility: turn already-read environment values into a `SessionKey`. No I/O and no `std::env`, so it is unit-testable without touching `environ`.
- `src/tools/rendezvous.rs` — publish / read / GC of `$XDG_STATE_HOME/codescout/servers/<pid>.json`. One responsibility: the on-disk handshake with the companion hook.

**codescout — modified:**

- `src/tools/mod.rs` — declare the two new modules.
- `src/server.rs` — `ServerEnv` gains the raw inputs; `from_parts_with_env` resolves the key (`:261-294`) and publishes the rendezvous entry; `CodeScoutServer` gains both handles.
- `src/tools/guide_ledger.rs` — `DEFAULT_IDLE_TTL_SECS`, `anonymous()`, `tick()`, `rekey()`, the `idle_ttl` field.
- `src/tools/core/types.rs` — around `:691`, poll the rendezvous and tick the TTL before reading the ledger.
- `src/prompts/guides/workspace-state.md` — **only** the storage/identity sentences. The activation-semantics sentence at `:51` belongs to Phase C.

**companion — modified:**

- `hooks/session-start.mjs` — after the existing `cc_session_id` write (`:30-37`), enumerate the rendezvous directory and stamp matching entries.
- `hooks/session-start.test.sh` — new assertions using the existing `ctx()` harness.

---

### Task 1: Session key resolution

**Files:**
- Create: `src/tools/session_key.rs`
- Modify: `src/tools/mod.rs`, `src/server.rs:55-67` (`ServerEnv`), `src/server.rs:261-294` (`from_parts_with_env`)
- Test: `src/tools/session_key.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `pub enum SessionKey { Keyed { id: String, source: KeySource }, Anonymous }`; `pub enum KeySource { Explicit, Harness }`; `SessionKey::id(&self) -> Option<&str>`; `pub const HARNESS_SESSION_VARS: &[&str]`; `pub fn resolve<I>(explicit: Option<String>, harness: I) -> SessionKey where I: IntoIterator<Item = (&'static str, String)>`. Tasks 2, 4 and 5 all branch on `SessionKey`.

**Context an implementer cannot infer:** today's chain lives at `src/server.rs:261-273` and reads `CLAUDE_CODE_SESSION_ID` → `<project_root>/.codescout/cc_session_id` → `uuid::Uuid::new_v4()`. Both fallbacks are being removed *from the ledger path only*. The file is per-project, so two Claude Code windows open on one repo overwrite each other's id — that is the 2026-08-16 attribution bug. The UUID tail is worse than it looks: it does not merely fail to dedup, it persists a ledger under a key nothing will ever match again, minting a dead file on every start.

- [ ] **Step 1: Write the failing tests**

Create `src/tools/session_key.rs`:

```rust
//! Conversation-identity resolution for the guide ledger.
//!
//! Deliberately free of I/O and `std::env`: the resolution ORDER is the part
//! worth testing, and tests must never mutate `environ` (see `ServerEnv`).

/// Which link produced the id. Carried for logging, not behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    /// `CODESCOUT_SESSION_ID` — the operator asserting an identity.
    Explicit,
    /// A known harness variable, e.g. `CLAUDE_CODE_SESSION_ID`.
    Harness,
}

/// A conversation identity, or the documented absence of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionKey {
    /// Tier 1: persist the ledger under `id`.
    Keyed { id: String, source: KeySource },
    /// Tier 2: no identity exists. In-process only, bounded by an idle TTL.
    Anonymous,
}

impl SessionKey {
    /// The id to key storage by, if any.
    pub fn id(&self) -> Option<&str> {
        match self {
            SessionKey::Keyed { id, .. } => Some(id),
            SessionKey::Anonymous => None,
        }
    }
}

/// Harness variables probed, in order. Extend by adding one entry; nothing else
/// in the chain changes. Probed unconditionally — never gated on `clientInfo`.
pub const HARNESS_SESSION_VARS: &[&str] = &["CLAUDE_CODE_SESSION_ID"];

/// First non-empty wins: explicit, then each harness var in order, then
/// `Anonymous`. Values are trimmed; whitespace-only counts as absent.
pub fn resolve<I>(explicit: Option<String>, harness: I) -> SessionKey
where
    I: IntoIterator<Item = (&'static str, String)>,
{
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_explicit_id_wins_over_every_harness_var() {
        let key = resolve(
            Some("explicit-1".to_string()),
            [("CLAUDE_CODE_SESSION_ID", "harness-1".to_string())],
        );
        assert_eq!(
            key,
            SessionKey::Keyed { id: "explicit-1".to_string(), source: KeySource::Explicit }
        );
    }

    #[test]
    fn a_harness_var_is_used_when_no_explicit_id_is_set() {
        let key = resolve(None, [("CLAUDE_CODE_SESSION_ID", "harness-1".to_string())]);
        assert_eq!(
            key,
            SessionKey::Keyed { id: "harness-1".to_string(), source: KeySource::Harness }
        );
    }

    #[test]
    fn harness_vars_are_probed_in_order_and_the_first_non_empty_wins() {
        // Kills a mutation that collects into a set, or takes the LAST match.
        let key = resolve(None, [("FIRST", "a".to_string()), ("SECOND", "b".to_string())]);
        assert_eq!(key.id(), Some("a"));
    }

    #[test]
    fn a_blank_or_whitespace_value_counts_as_absent_at_every_rank() {
        // Kills a mutation checking `is_some()` rather than non-empty-after-trim.
        let key = resolve(
            Some("   ".to_string()),
            [("A", String::new()), ("B", "\t\n".to_string()), ("C", "real".to_string())],
        );
        assert_eq!(key.id(), Some("real"));
    }

    #[test]
    fn an_id_is_trimmed_before_use() {
        // A trailing newline is what a file-written id looks like; keying on the
        // untrimmed form silently splits one conversation across two ledgers.
        let key = resolve(Some("  abc-123\n".to_string()), []);
        assert_eq!(key.id(), Some("abc-123"));
    }

    #[test]
    fn no_source_at_all_is_anonymous_not_a_generated_id() {
        // Kills the old `unwrap_or_else(|| Uuid::new_v4())` tail, which persisted
        // a ledger under a key nothing could ever match again.
        assert_eq!(resolve(None, []), SessionKey::Anonymous);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib session_key`
Expected: FAIL — `not implemented` panic from `unimplemented!()` in all six tests.

- [ ] **Step 3: Implement `resolve`**

Replace the `unimplemented!()` body:

```rust
pub fn resolve<I>(explicit: Option<String>, harness: I) -> SessionKey
where
    I: IntoIterator<Item = (&'static str, String)>,
{
    fn clean(v: String) -> Option<String> {
        let t = v.trim();
        (!t.is_empty()).then(|| t.to_string())
    }

    if let Some(id) = explicit.and_then(clean) {
        return SessionKey::Keyed { id, source: KeySource::Explicit };
    }
    for (_name, value) in harness {
        if let Some(id) = clean(value) {
            return SessionKey::Keyed { id, source: KeySource::Harness };
        }
    }
    SessionKey::Anonymous
}
```

Add to `src/tools/mod.rs`, in the existing alphabetical run of `pub mod` declarations:

```rust
pub mod session_key;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib session_key`
Expected: PASS, 6 tests.

- [ ] **Step 5: Add the raw inputs to `ServerEnv`**

In `src/server.rs`, keep `cc_session_id` (`:58-59`) for its usage-correlation job and add the new inputs beside it, updating its doc comment so the split is legible:

```rust
    /// `CLAUDE_CODE_SESSION_ID` — correlation id for `usage.db`. NOT the ledger
    /// key: see `session_id_explicit` / `harness_session_ids`, which resolve the
    /// ledger's identity under different collision requirements.
    pub cc_session_id: Option<String>,
    /// `CODESCOUT_SESSION_ID` — rank 1 of the ledger key chain. Trusted when set;
    /// documented as unique-per-conversation, since a value pinned in MCP config
    /// is constant across every conversation in that project.
    pub session_id_explicit: Option<String>,
    /// `(name, value)` for each of `session_key::HARNESS_SESSION_VARS` that is
    /// set, in probe order. Captured as data so tests inject without `set_var`.
    pub harness_session_ids: Vec<(&'static str, String)>,
```

In `ServerEnv::from_env` (around `:77`):

```rust
            session_id_explicit: std::env::var("CODESCOUT_SESSION_ID").ok(),
            harness_session_ids: crate::tools::session_key::HARNESS_SESSION_VARS
                .iter()
                .filter_map(|name| std::env::var(name).ok().map(|v| (*name, v)))
                .collect(),
```

- [ ] **Step 6: Resolve the key at construction**

In `from_parts_with_env`, leave the existing `cc_session_id` binding (`:261-273`) exactly as it is — `UsageRecorder` at `:787-791` still consumes it. Add immediately after:

```rust
        // The ledger's key is resolved separately from the usage-correlation id
        // above: usage tolerates a collision between two windows on one repo,
        // the ledger does not. See the plan's Ruling 3 and spec §1.
        let session_key = crate::tools::session_key::resolve(
            env.session_id_explicit.clone(),
            env.harness_session_ids.clone(),
        );
        if session_key.id().is_none() {
            tracing::info!(
                "no conversation id available (checked CODESCOUT_SESSION_ID and {:?}); \
                 guide ledger is in-process only and re-arms after idle",
                crate::tools::session_key::HARNESS_SESSION_VARS,
            );
        }
```

Change the `GuideLedger::load` call at `:294`. Task 2 supplies the real anonymous branch; for now:

```rust
        let guide_hints_emitted = Arc::new(parking_lot::Mutex::new(match session_key.id() {
            Some(id) => crate::tools::guide_ledger::GuideLedger::load(id, guide_hints_dir),
            None => crate::tools::guide_ledger::GuideLedger::load("", None),
        }));
```

Store `session_key` on `CodeScoutServer` beside `cc_session_id` (`:115`) — Tasks 2 and 5 need it:

```rust
    /// Resolved conversation identity for the guide ledger. Distinct from
    /// `cc_session_id`, which is usage-correlation only.
    session_key: crate::tools::session_key::SessionKey,
```

- [ ] **Step 7: Fix the test that keyed off the old field**

`guide_ledger_survives_mcp_restart` (`src/server.rs:4385`) injects `cc_session_id: Some(session)`. That field no longer keys the ledger, so the test would pass while testing nothing. Change it to set `session_id_explicit: Some(session.to_string())`. Fixing this is part of Task 1 — a test left green by accident is worse than a red one.

- [ ] **Step 8: Run the gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git commit -m "feat(guide-ledger): resolve a conversation identity, or say there is none

Replaces the per-project file and random-uuid fallbacks with an explicit
Anonymous tier. The file collided between concurrent windows; the uuid
persisted a ledger under a key nothing could ever match again." -- src/tools/session_key.rs src/tools/mod.rs src/server.rs
```

---

### Task 2: Anonymous-tier idle TTL

**Files:**
- Modify: `src/tools/guide_ledger.rs`, `src/tools/core/types.rs:691`, `src/server.rs`
- Test: `src/tools/guide_ledger.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `SessionKey` (Task 1).
- Produces: `pub const DEFAULT_IDLE_TTL_SECS: u64 = 7200`; `GuideLedger::anonymous(idle_ttl: Option<Duration>) -> Self`; `GuideLedger::tick(&mut self) -> usize`.

**Context an implementer cannot infer:** `expire_idle` (`src/tools/guide_ledger.rs:181-193`) already exists and works, with **zero production callers** — Phase A built the mechanism and deliberately left it unwired. This task is its first caller. Do not reimplement or modify `expire_idle` itself.

Why 2 hours, and why only for the anonymous tier: measured over 258 sessions, a 2h TTL fires spuriously on 34.5% of *live* conversations, and the curve is flat — reaching 5% costs a 48-hour window, which is a worthless starvation bound. For tier 1 the rendezvous is the mechanism, so no TTL is used at all. For tier 2 starvation is the *default and permanent*: without a TTL, every conversation after the first in a long-lived process gets no guides, ever. A spurious re-injection inside a process that never persists anything is the correct error. Spec §7.

- [ ] **Step 1: Write the failing tests**

Add to the existing `mod tests` in `src/tools/guide_ledger.rs`:

```rust
    #[test]
    fn an_anonymous_ledger_never_persists_even_with_a_ttl() {
        // Tier 2's whole contract: in-process only. A path here would mint files
        // under a key nothing can ever match.
        let mut l = GuideLedger::anonymous(Some(Duration::from_secs(7200)));
        l.insert("librarian".to_string());
        assert!(l.contains("librarian"));
        assert!(l.path_for_test().is_none(), "anonymous ledger must have no path");
    }

    #[test]
    fn tick_expires_topics_older_than_the_ttl_and_leaves_fresh_ones() {
        let mut l = GuideLedger::anonymous(Some(Duration::from_secs(3600)));
        l.insert("stale".to_string());
        l.insert("fresh".to_string());
        l.backdate_for_test("stale", chrono::Duration::hours(2));
        assert_eq!(l.tick(), 1);
        assert!(!l.contains("stale"));
        assert!(l.contains("fresh"));
    }

    #[test]
    fn tick_on_a_ledger_with_no_ttl_expires_nothing_however_old() {
        // Tier 1 must never expire by time — the rendezvous is its mechanism.
        // Kills a mutation applying DEFAULT_IDLE_TTL_SECS unconditionally.
        let mut l = GuideLedger::anonymous(None);
        l.insert("ancient".to_string());
        l.backdate_for_test("ancient", chrono::Duration::days(30));
        assert_eq!(l.tick(), 0);
        assert!(l.contains("ancient"));
    }

    #[test]
    fn a_keyed_ledger_loaded_from_disk_has_no_ttl_by_default() {
        // Kills a mutation giving every ledger the anonymous TTL.
        let dir = tempfile::tempdir().unwrap();
        let mut l = GuideLedger::load("s1", Some(dir.path().to_path_buf()));
        l.insert("librarian".to_string());
        l.backdate_for_test("librarian", chrono::Duration::days(30));
        assert_eq!(l.tick(), 0, "tier 1 must not expire by time");
    }
```

Add the two test-only helpers next to `stamps_for_test` (`:93-95`):

```rust
    /// The backing path, for tests asserting a ledger is ephemeral.
    #[cfg(test)]
    pub fn path_for_test(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    /// Push a topic's stamp back in time, so expiry is testable without sleeping.
    #[cfg(test)]
    pub fn backdate_for_test(&mut self, topic: &str, by: chrono::Duration) {
        if let Some(at) = self.emitted.get_mut(topic) {
            *at -= by;
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib guide_ledger`
Expected: FAIL to compile — `no function or associated item named 'anonymous'`, `no method named 'tick'`.

- [ ] **Step 3: Implement**

Add near `GC_MAX_IDLE_DAYS` (`:28`):

```rust
/// Idle window after which an anonymous-tier topic re-arms. Two hours: measured
/// over 258 sessions this fires spuriously on 34.5% of live conversations, but
/// the alternative for a client with no conversation identity is permanent
/// starvation of every conversation after the first. Spec §7.
pub const DEFAULT_IDLE_TTL_SECS: u64 = 7200;
```

Add the field to `GuideLedger` (`:38-61`):

```rust
    /// Anonymous tier only. `None` ⇒ never expire by time.
    idle_ttl: Option<std::time::Duration>,
```

Every existing constructor must set `idle_ttl: None` — `load` (`:76-89`) and the derived `Default`. Then add:

```rust
    /// A ledger for a client exposing no conversation identity: in-process only,
    /// bounded by `idle_ttl` rather than keyed by a session.
    pub fn anonymous(idle_ttl: Option<std::time::Duration>) -> Self {
        Self { path: None, emitted: Default::default(), notices: HashSet::new(), idle_ttl }
    }

    /// Apply the configured idle TTL, if any. Returns how many topics re-armed.
    /// Cheap enough for every guide-eligible request: a `BTreeMap::retain` over a
    /// handful of entries, persisting only on an actual change.
    pub fn tick(&mut self) -> usize {
        match self.idle_ttl {
            Some(ttl) => self.expire_idle(ttl),
            None => 0,
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib guide_ledger`
Expected: PASS — the 4 new tests plus all 19 existing ones.

- [ ] **Step 5: Call `tick()` on the request path**

In `src/tools/core/types.rs`, at the ledger lock (around `:691`), tick before the emptiness check:

```rust
        let hint_topic: Option<String> = {
            let mut emitted = ctx.guide_hints_emitted.lock();
            // Anonymous tier only (a no-op when no TTL is configured): re-arm
            // topics the model plausibly no longer holds. Must run BEFORE
            // is_empty(), or an expiry that empties the ledger goes unseen until
            // the next call.
            emitted.tick();
            if emitted.is_empty() {
```

- [ ] **Step 6: Construct the anonymous ledger in the server**

In `src/server.rs`, replace the placeholder `None` arm from Task 1 Step 6:

```rust
        let idle_ttl = env.guide_idle_ttl.unwrap_or(std::time::Duration::from_secs(
            crate::tools::guide_ledger::DEFAULT_IDLE_TTL_SECS,
        ));
        let guide_hints_emitted = Arc::new(parking_lot::Mutex::new(match session_key.id() {
            Some(id) => crate::tools::guide_ledger::GuideLedger::load(id, guide_hints_dir),
            // Zero is an explicit operator opt-out, accepting starvation.
            None => crate::tools::guide_ledger::GuideLedger::anonymous(
                (!idle_ttl.is_zero()).then_some(idle_ttl),
            ),
        }));
```

Add to `ServerEnv`:

```rust
    /// `CODESCOUT_GUIDE_TTL_SECS` — anonymous-tier idle window. `None` ⇒ the
    /// default; `Some(0)` ⇒ no expiry at all, an explicit opt-out.
    pub guide_idle_ttl: Option<std::time::Duration>,
```

and in `from_env`:

```rust
            guide_idle_ttl: std::env::var("CODESCOUT_GUIDE_TTL_SECS")
                .ok()
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(std::time::Duration::from_secs),
```

An unparseable value falls back to the default rather than erroring — a typo'd env var must not brick guide delivery.

- [ ] **Step 7: Run the gate and commit**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: PASS.

```bash
git commit -m "feat(guide-ledger): bound the anonymous tier by idle time

expire_idle has existed since Phase A with no production caller. This is it:
clients with no conversation identity re-arm after 2h idle instead of starving
every conversation after the first." -- src/tools/guide_ledger.rs src/tools/core/types.rs src/server.rs
```

---

### Task 3: Pin the fully-expired-ledger opener re-fire

**Files:**
- Modify: `src/tools/guide_ledger.rs` (doc comment on `persist`), `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` (§7 subsection)
- Test: `src/tools/guide_ledger.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `backdate_for_test` (Task 2).
- Produces: nothing later tasks consume. This task is a decision plus its regression guard.

**Context an implementer cannot infer:** the spec's §7 subsection "Open for Phase B: expiring the *last* topic re-fires the session opener" describes an emergent chain nothing pins. `expire_idle` removes the final topic → `persist` (`:219-233`) takes its empty-map branch and **deletes** the file → the next `load` yields an empty ledger → `is_empty()` (`:108-110`) is the trigger for `SESSION_OPENING_GUIDE` at `src/tools/core/types.rs:692`. The decision — confirmed by the user 2026-08-18 — is to **accept** it: after a full idle TTL the model plausibly holds none of the orientation anyway, and the governing invariant is "degrade to re-sending, never to suppressing." This task turns an accident into a documented, tested decision. Do **not** change `persist`'s behaviour.

- [ ] **Step 1: Write the test**

```rust
    #[test]
    fn expiring_the_last_topic_deletes_the_file_so_the_session_opener_re_fires() {
        // DECISION, not an accident (spec §7, confirmed 2026-08-18): a fully
        // expired ledger re-opens the session. `is_empty()` is the opener's
        // trigger (src/tools/core/types.rs:692), and persist deletes the file
        // when the map empties — so a reload is empty and the opener fires.
        // Pinned because both halves look like reasonable refactors: making
        // persist write `{}` instead of deleting would suppress the opener
        // permanently, and nothing else would catch it.
        let dir = tempfile::tempdir().unwrap();
        let mut l = GuideLedger::load("s-expire", Some(dir.path().to_path_buf()));
        l.insert("librarian".to_string());
        let path = dir.path().join("s-expire.json");
        assert!(path.exists(), "precondition: the ledger was persisted");

        l.backdate_for_test("librarian", chrono::Duration::hours(3));
        assert_eq!(l.expire_idle(Duration::from_secs(3600)), 1);

        assert!(!path.exists(), "an emptied ledger must remove its file");
        let reloaded = GuideLedger::load("s-expire", Some(dir.path().to_path_buf()));
        assert!(reloaded.is_empty(), "a reloaded empty ledger must re-fire the opener");
    }
```

- [ ] **Step 2: Run it, then prove it is not vacuous**

Run: `cargo test --lib expiring_the_last_topic`
Expected: PASS immediately — this test pins existing behaviour rather than driving new code.

Because it passes on arrival, it must be shown to fail against the mutation it exists to catch. Temporarily change `persist`'s empty branch to write `"{}"` instead of `remove_file`, re-run, and confirm it FAILS on both `!path.exists()` and `reloaded.is_empty()`. **Revert the mutation.** Paste the actual failure output into the task report — an asserted-but-unrun mutation is not evidence, and a report claiming otherwise will be rejected at review.

- [ ] **Step 3: Document the decision at the code**

Extend `persist`'s doc comment (`:212-218`) with:

```rust
    /// Deleting on empty is load-bearing beyond tidiness: `is_empty()` is what
    /// fires `SESSION_OPENING_GUIDE` (`src/tools/core/types.rs:692`), so a ledger
    /// emptied by `expire_idle` re-opens the session on its next load. That is
    /// intended for a client idle past its TTL — spec §7. Writing `{}` here
    /// instead would suppress the opener permanently.
```

- [ ] **Step 4: Close the open question in the spec**

With `edit_markdown`, rewrite the body of §7's subsection "Open for Phase B: expiring the *last* topic re-fires the session opener" to record the resolution: decided 2026-08-18 to accept; pinned by `expiring_the_last_topic_deletes_the_file_so_the_session_opener_re_fires`; rationale is the governing invariant. Keep the heading but mark it resolved, so the spec no longer reads as open.

- [ ] **Step 5: Run the gate and commit**

```bash
git commit -m "test(guide-ledger): pin the fully-expired ledger re-opening the session

Spec section 7 left this open. Decided: accept it. Both halves of the chain
look like reasonable refactors, so it needs a guard rather than a comment." -- src/tools/guide_ledger.rs docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md
```

---

### Task 4: Publish the rendezvous entry

**Files:**
- Create: `src/tools/rendezvous.rs`
- Modify: `src/tools/mod.rs`, `src/server.rs` (`ServerEnv`, `from_parts_with_env`)
- Test: `src/tools/rendezvous.rs` (inline `mod tests`)

**Interfaces:**
- Consumes: `SessionKey::id()` (Task 1).
- Produces: `pub struct Entry { pid, ppid, started_at, cwd, session, hook_at }`; `pub struct Rendezvous`; `Rendezvous::publish(dir: Option<PathBuf>, session: Option<&str>) -> Self`; `Rendezvous::path(&self) -> Option<&Path>`. Task 5 adds the read side to this same struct.

**Context an implementer cannot infer — read all of this before writing code:**

MCP `initialize` runs **before** `SessionStart`, so a hook cannot mint an id the server reads at startup. The server publishes a slot first; the hook writes into it. Keyed by **server pid**, deliberately: a per-project file is what caused the 2026-08-16 attribution bug, and two concurrent windows on one repo would collide again. A pid is a valid rendezvous *within one process lifetime* even though it is useless as durable identity — that distinction is the whole reason this is safe where a PPID-derived key was not.

**Publish ONLY from the MCP server construction path.** Measured on the development host 2026-08-18: 25 codescout processes were alive, two of which (`codescout mux --socket …`) are LSP multiplexers whose parent is another codescout, not the harness. They never serve `get_guide`. If publishing happens in any shared init that `mux` also runs, every mux process mints a `<pid>.json` no hook can ever match — permanent stale entries, indistinguishable from dead servers. `from_parts_with_env` is the correct and only site. Recorded as W-47 in `docs/trackers/bug-fix-session-log.md`.

Ancestry, verified live on the same host: server `pid=1844342` had `ppid=2417823`, the `claude` process that also spawns the hook. So `ppid` is the field the hook matches on.

GC by liveness is free — `crate::platform::process_alive(pid: u32) -> bool` already exists cross-platform (`src/platform/mod.rs:47`, `unix.rs:103`, `windows.rs:299`). Use it; do not add a dependency or write per-platform code.

- [ ] **Step 1: Write the failing tests**

Create `src/tools/rendezvous.rs` with the types, `unimplemented!()` bodies, and:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn entry_at(dir: &std::path::Path, pid: u32) -> Option<Entry> {
        let text = std::fs::read_to_string(dir.join(format!("{pid}.json"))).ok()?;
        serde_json::from_str(&text).ok()
    }

    #[test]
    fn publish_writes_an_entry_named_for_this_process() {
        let dir = tempfile::tempdir().unwrap();
        let r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("sess-1"));
        let me = std::process::id();
        let e = entry_at(dir.path(), me).expect("entry must exist under our own pid");
        assert_eq!(e.pid, me);
        assert_eq!(e.session.as_deref(), Some("sess-1"));
        assert!(e.hook_at.is_none(), "a freshly published entry is unstamped");
        assert!(r.path().is_some());
    }

    #[test]
    fn publish_records_the_parent_pid_the_hook_matches_on() {
        // The hook selects entries whose ppid is on its own ancestry. A zero or
        // missing ppid makes every entry unmatchable — the feature silently dies.
        let dir = tempfile::tempdir().unwrap();
        Rendezvous::publish(Some(dir.path().to_path_buf()), None);
        let e = entry_at(dir.path(), std::process::id()).unwrap();
        assert_ne!(e.ppid, 0, "ppid must be recorded");
    }

    #[test]
    fn publish_with_no_directory_is_inert_and_never_panics() {
        let r = Rendezvous::publish(None, Some("sess-1"));
        assert!(r.path().is_none());
    }

    #[test]
    fn publish_collects_entries_whose_process_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        // Pid 0 is never a live user process on any supported platform.
        std::fs::write(
            dir.path().join("0.json"),
            r#"{"pid":0,"ppid":1,"started_at":"2026-01-01T00:00:00Z","cwd":"/","session":null,"hook_at":null}"#,
        )
        .unwrap();
        Rendezvous::publish(Some(dir.path().to_path_buf()), None);
        assert!(!dir.path().join("0.json").exists(), "a dead pid's entry must be collected");
    }

    #[test]
    fn publish_keeps_the_entry_of_a_live_process() {
        // Kills a mutation that collects the whole directory rather than dead pids.
        let dir = tempfile::tempdir().unwrap();
        let live = std::process::id();
        std::fs::write(
            dir.path().join(format!("{live}.json")),
            format!(r#"{{"pid":{live},"ppid":1,"started_at":"2026-01-01T00:00:00Z","cwd":"/","session":"old","hook_at":null}}"#),
        )
        .unwrap();
        Rendezvous::publish(Some(dir.path().to_path_buf()), Some("new"));
        let e = entry_at(dir.path(), live).expect("a live pid's entry must survive");
        assert_eq!(e.session.as_deref(), Some("new"), "our own entry is rewritten");
    }

    #[test]
    fn publish_ignores_non_json_and_unparseable_files() {
        // The directory is shared; a stray file must never panic or be deleted.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.txt"), "hello").unwrap();
        std::fs::write(dir.path().join("garbage.json"), "{{{{").unwrap();
        Rendezvous::publish(Some(dir.path().to_path_buf()), None);
        assert!(dir.path().join("notes.txt").exists(), "non-json must be left alone");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib rendezvous`
Expected: FAIL — `unimplemented!()`.

- [ ] **Step 3: Implement**

```rust
//! The pid-keyed handshake that lets a companion hook push a fresh conversation
//! id into an already-running server.
//!
//! MCP `initialize` runs before `SessionStart`, so the hook cannot mint an id the
//! server reads at startup: the server publishes a slot, the hook writes into it.
//! Keyed by server pid because a per-project file collides between two windows on
//! one repo — the 2026-08-16 attribution bug. A pid is a valid rendezvous within
//! one process lifetime even though it is useless as durable identity.
//!
//! Published ONLY from `CodeScoutServer::from_parts_with_env`. `codescout mux`
//! processes are children of a codescout, never of the harness, and never serve
//! guides — publishing from any shared init would mint entries no hook can match.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One server's slot. `session` is what the server reads back; `hook_at` is how
/// it knows a hook — rather than only itself — has written here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub pid: u32,
    pub ppid: u32,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub cwd: String,
    pub session: Option<String>,
    /// Set by the companion hook. `None` ⇒ no rendezvous is active.
    pub hook_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct Rendezvous {
    path: Option<PathBuf>,
}

impl Rendezvous {
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Write this process's slot, and collect slots whose process is gone.
    /// Best-effort throughout: a failure costs the `/clear` refresh, never
    /// correctness — the idle TTL and the next restart both still work.
    pub fn publish(dir: Option<PathBuf>, session: Option<&str>) -> Self {
        let Some(dir) = dir else { return Self { path: None } };
        if std::fs::create_dir_all(&dir).is_err() {
            return Self { path: None };
        }
        gc(&dir);
        let pid = std::process::id();
        let entry = Entry {
            pid,
            ppid: parent_pid(),
            started_at: chrono::Utc::now(),
            cwd: std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            session: session.map(str::to_string),
            hook_at: None,
        };
        let path = dir.join(format!("{pid}.json"));
        match serde_json::to_string(&entry) {
            Ok(json) => match crate::util::fs::write_utf8(&path, &json) {
                Ok(()) => Self { path: Some(path) },
                Err(e) => {
                    tracing::debug!("rendezvous publish failed ({}): {e}", path.display());
                    Self { path: None }
                }
            },
            Err(e) => {
                tracing::debug!("rendezvous serialize failed: {e}");
                Self { path: None }
            }
        }
    }
}

#[cfg(unix)]
fn parent_pid() -> u32 {
    // SAFETY: getppid takes no arguments and cannot fail.
    unsafe { libc::getppid() as u32 }
}

#[cfg(windows)]
fn parent_pid() -> u32 {
    // No getppid here. The hook walks ancestry itself, so a zero degrades to
    // "never matched" rather than to a WRONG match — the safe direction.
    0
}

/// Remove slots whose process is gone. Marked cleanup, not discovery — see
/// `src/lsp/mux/mod.rs:68-72` (R-45) for why that distinction is written down.
fn gc(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(pid) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        if !crate::platform::process_alive(pid) {
            let _ = std::fs::remove_file(&path);
        }
    }
}
```

Declare `pub mod rendezvous;` in `src/tools/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib rendezvous`
Expected: PASS, 6 tests.

- [ ] **Step 5: Publish at server construction**

In `from_parts_with_env`, after `guide_hints_dir` is bound (`:290-293`), add the sibling directory and publish:

```rust
        let servers_dir = env.servers_dir.clone().or_else(|| {
            crate::util::fs::per_user_state_dir().map(|d| d.join("codescout").join("servers"))
        });
        let rendezvous = crate::tools::rendezvous::Rendezvous::publish(servers_dir, session_key.id());
```

Add `pub servers_dir: Option<PathBuf>` to `ServerEnv`, with the same test-injection rationale Phase A wrote for `guide_hints_dir`, and store `rendezvous` on `CodeScoutServer` for Task 5.

**Every existing test that builds a server must inject `servers_dir`** pointing at a tempdir, exactly as Phase A did for `guide_hints_dir` — otherwise the suite writes into and garbage-collects the developer's real `~/.local/state/codescout/servers`. Extend Phase A's `test_env(dir)` helper rather than touching 16 call sites.

- [ ] **Step 6: Run the gate and commit**

```bash
git commit -m "feat(rendezvous): publish a pid-keyed slot for the companion to stamp

Published only from the MCP construction path: codescout mux processes are
children of a codescout, never the harness, so a shared init site would mint
entries no hook could ever match (W-47)." -- src/tools/rendezvous.rs src/tools/mod.rs src/server.rs
```

---

### Task 5: Read the rendezvous and re-arm on a session change

**Files:**
- Modify: `src/tools/rendezvous.rs`, `src/tools/guide_ledger.rs` (`rekey`), `src/server.rs`, `src/tools/core/types.rs`
- Test: `src/tools/rendezvous.rs`, `src/server.rs` (`guide_hint_tests`)

**Interfaces:**
- Consumes: `Rendezvous`, `Entry` (Task 4); `GuideLedger` internals (Task 2).
- Produces: `Rendezvous::poll(&mut self) -> Option<String>` — the new session id, only when it changed; `Rendezvous::is_active(&self) -> bool`; `GuideLedger::rekey(&mut self, session: &str)`. **Phase C gates its re-arm predicate on `is_active()`** — that is this task's contract with the next plan, and the reason `is_active` is public despite having no in-phase consumer.

**Context an implementer cannot infer:** the server re-reads its own entry only when the file's mtime changes — a `metadata` call per request, not a parse. On a session change it re-arms the **whole** ledger, because a new conversation holds nothing, and repoints storage at the new id. The server must not depend on the hook: no hook installed → `hook_at` stays `None` → `is_active()` is false → the key never refreshes, and the anonymous-tier TTL is what eventually catches `/clear`, one interval late. That is the Agent-Agnostic contract — the companion *adds* enforcement, the server *degrades* without it.

**One edge the spec does not cover, decided here.** An *anonymous* ledger (no path) can still receive a hook stamp: Claude Code versions before v2.1.154 run the companion hook but do not set `CLAUDE_CODE_SESSION_ID`, so the env chain yields `Anonymous` while a session id arrives through the rendezvous. `rekey` on a path-less ledger therefore clears in-memory state and stays path-less rather than promoting to keyed. The session change still re-arms correctly; only cross-restart persistence is missing, which is exactly what that Claude Code version could never have had anyway. Promoting anonymous→keyed mid-process is deliberately out of scope — it would mean re-running GC and load against a directory the process had already decided not to touch.

- [ ] **Step 1: Write the failing tests**

In `src/tools/rendezvous.rs`, add the hook-mimicking helper and the polling tests:

```rust
    /// Rewrite an entry the way the companion hook does: new session, hook_at set.
    fn stamp_as_hook(path: &std::path::Path, session: &str) {
        let mut e: Entry = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        e.session = Some(session.to_string());
        e.hook_at = Some(chrono::Utc::now());
        std::fs::write(path, serde_json::to_string(&e).unwrap()).unwrap();
        // mtime resolution is coarse on some filesystems; make the change visible.
        filetime::set_file_mtime(path, filetime::FileTime::from_unix_time(2_000_000_000, 0)).unwrap();
    }

    #[test]
    fn poll_reports_a_session_written_by_the_hook() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("old"));
        stamp_as_hook(r.path().unwrap(), "new");
        assert_eq!(r.poll().as_deref(), Some("new"));
        assert!(r.is_active(), "a hook stamp activates the rendezvous");
    }

    #[test]
    fn poll_returns_none_when_nothing_changed() {
        // Kills a mutation that re-arms on every call — which would wipe the
        // ledger continuously and defeat the entire feature while looking fine.
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("old"));
        stamp_as_hook(r.path().unwrap(), "new");
        assert_eq!(r.poll().as_deref(), Some("new"));
        assert_eq!(r.poll(), None, "a second poll with no new write must be quiet");
    }

    #[test]
    fn poll_ignores_a_stamp_repeating_the_session_we_already_have() {
        // Every SessionStart stamps, including `resume` on an unchanged
        // conversation. Re-arming there would punish resuming a session.
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("same"));
        stamp_as_hook(r.path().unwrap(), "same");
        assert_eq!(r.poll(), None);
    }

    #[test]
    fn an_unstamped_entry_is_not_active_and_polls_quiet() {
        // The no-companion path. Must degrade, never mis-fire.
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("s"));
        assert!(!r.is_active());
        assert_eq!(r.poll(), None);
    }

    #[test]
    fn a_corrupt_or_deleted_entry_polls_quiet_rather_than_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = Rendezvous::publish(Some(dir.path().to_path_buf()), Some("s"));
        std::fs::write(r.path().unwrap(), "{{{{").unwrap();
        assert_eq!(r.poll(), None);
        std::fs::remove_file(r.path().unwrap()).unwrap();
        assert_eq!(r.poll(), None);
    }
```

If `filetime` is not already a dev-dependency, drop that line and instead write the file twice with a `std::thread::sleep(Duration::from_millis(10))` between — but prefer `filetime` if present, since a sleeping test is a flaky test.

In `src/server.rs`'s `guide_hint_tests`, the spec's named test. Follow the shape of `guide_ledger_survives_mcp_restart` (`:4378-4420`), which is the closest sibling:

```rust
    #[tokio::test]
    /// A new conversation holds nothing, so a session change re-arms the WHOLE
    /// ledger — not just the project-scoped topic. This is the `/clear` fix:
    /// docs/issues/2026-08-18-clear-leaves-mcp-session-id-stale.md.
    ///
    /// No `#[serial]`, no `set_var`: session id and directories are INJECTED.
    async fn session_change_rearms_everything() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let servers = tempfile::tempdir().unwrap();

        let env = ServerEnv {
            session_id_explicit: Some("conv-A".to_string()),
            servers_dir: Some(servers.path().to_path_buf()),
            librarian: crate::librarian::LibrarianEnv {
                db: Some(dir.path().join("librarian.db")),
                ..Default::default()
            },
            ..test_env(dir.path())
        };
        let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
            .await
            .unwrap();
        let server =
            CodeScoutServer::from_parts_with_env(agent, LspManager::new_arc(), false, env).await;

        server.guide_hints_emitted.lock().insert("librarian".to_string());
        server.guide_hints_emitted.lock().insert("progressive-disclosure".to_string());

        // The companion hook stamps our slot with a DIFFERENT conversation.
        let slot = servers.path().join(format!("{}.json", std::process::id()));
        let mut entry: crate::tools::rendezvous::Entry =
            serde_json::from_str(&std::fs::read_to_string(&slot).unwrap()).unwrap();
        entry.session = Some("conv-B".to_string());
        entry.hook_at = Some(chrono::Utc::now());
        std::fs::write(&slot, serde_json::to_string(&entry).unwrap()).unwrap();

        server.rendezvous_poll_for_test();

        let ledger = server.guide_hints_emitted.lock();
        assert!(!ledger.contains("librarian"), "a new conversation re-arms every topic");
        assert!(
            !ledger.contains("progressive-disclosure"),
            "re-arm must be total, not just the project-scoped topic"
        );
    }
```

The second assertion is the one that matters: a mutation re-arming only `project-activation-bootstrap` — which is Phase C's *correct* behaviour for a project switch — is wrong here, and the first assertion alone would not catch it.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib rendezvous`, then `cargo test --lib session_change_rearms`.
Expected: compile failure — `no method named 'poll'` / `'is_active'` / `'rekey'` / `'rendezvous_poll_for_test'`.

- [ ] **Step 3: Implement `poll` and `is_active`**

Extend the struct from Task 4 and add the read side:

```rust
pub struct Rendezvous {
    path: Option<PathBuf>,
    /// Last mtime we parsed at. `None` ⇒ never read.
    last_mtime: Option<std::time::SystemTime>,
    /// The session we currently believe we are serving.
    current: Option<String>,
    /// Set once a hook has written here.
    active: bool,
}

impl Rendezvous {
    /// Has a companion hook written into our slot?
    ///
    /// Phase C gates its re-arm predicate on this: without a rendezvous the
    /// server cannot detect a conversation change, so the blunt
    /// clear-on-every-activate behaviour has to stay. Shipping the precise
    /// predicate ungated would remove the accidental mitigation for `/clear`
    /// without supplying the real one.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Returns the new session id ONLY when it changed.
    ///
    /// Called on every guide-eligible request, so the unchanged path must stay
    /// cheap: one `metadata` call, and no read or parse unless the mtime moved.
    pub fn poll(&mut self) -> Option<String> {
        let path = self.path.as_deref()?;
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
        if self.last_mtime == Some(mtime) {
            return None;
        }
        self.last_mtime = Some(mtime);
        let entry: Entry = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())?;
        if entry.hook_at.is_some() {
            self.active = true;
        }
        let session = entry.session?;
        if self.current.as_deref() == Some(session.as_str()) {
            return None;
        }
        self.current = Some(session.clone());
        Some(session)
    }
}
```

`publish` must initialise `current` to the session it wrote, `last_mtime: None`, and `active: false`. Initialising `current` is what makes the *first* poll quiet: it re-reads the file it just wrote, finds the same session, and returns `None`.

- [ ] **Step 4: Add `GuideLedger::rekey`**

It belongs beside the fields it mutates:

```rust
    /// Point this ledger at a different conversation, discarding everything the
    /// old one held. The old session's file is left alone: it may still belong
    /// to a live sibling process, and the 35-day GC collects it otherwise.
    ///
    /// An anonymous ledger has no path and stays path-less — see Task 5's note
    /// on pre-v2.1.154 Claude Code, where a hook stamp arrives without an env id.
    pub fn rekey(&mut self, session: &str) {
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            self.path = Some(dir.join(format!("{}.json", sanitize(session))));
        }
        self.emitted.clear();
        self.notices.clear();
    }
```

- [ ] **Step 5: Wire the request path**

In `src/tools/core/types.rs`, immediately before the ledger lock added in Task 2:

```rust
        if let Some(new_session) = ctx.rendezvous_poll() {
            ctx.guide_hints_emitted.lock().rekey(&new_session);
        }
```

`rekey` already clears, so no separate `clear()` call — a double clear would be harmless but would misdescribe the intent.

Expose `rendezvous_poll()` on the context (it needs `&mut` access to the `Rendezvous`, so hold it in a `parking_lot::Mutex` alongside the ledger), plus a `rendezvous_poll_for_test()` on `CodeScoutServer` that the Task-5 server test drives directly.

- [ ] **Step 6: Run the gate and commit**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: PASS.

```bash
git commit -m "feat(rendezvous): re-arm the ledger when the conversation changes

Closes the /clear defect: a new conversation id arrives without the MCP
subprocess respawning, so the ledger has to be told rather than reloaded." -- src/tools/rendezvous.rs src/tools/guide_ledger.rs src/server.rs src/tools/core/types.rs
```
---

### Task 6: Companion hook stamps matching slots

**Files — SEPARATE GIT REPO, `/home/marius/work/claude/claude-plugins/codescout-companion`:**
- Modify: `hooks/session-start.mjs`, `hooks/session-start.test.sh`

**Interfaces:**
- Consumes: the `Entry` JSON shape from Task 4 — `{pid, ppid, started_at, cwd, session, hook_at}`.
- Produces: entries with `session` and `hook_at` filled, which Task 5's `poll` reads.

**Context an implementer cannot infer:**

`session-start.mjs` is 327 lines. It reads its input via `readInput()` from `./lib.mjs`, which also exports `emit`, `detectFor`, `git`, `denyPreToolUse`, `contextPreToolUse`, and `emitSkillHint` — **reuse these, do not reimplement them.** The hook branches on `source` at `:254` (`if (source === 'compact')`), which is the precedent for source-sensitive behaviour — the rendezvous stamp is deliberately **not** source-gated, since `startup`, `resume`, `compact` and the post-`/clear` start all need it.

**Placement is load-bearing.** The natural-looking spot is right after the existing `cc_session_id` write at `:30-37`, but the block below reads `home`, which is `const`-declared at `:46`. A `const` referenced above its declaration throws `ReferenceError` from the temporal dead zone — and because the whole hook would then abort, the failure surfaces as *the session losing all its injected context*, not as a rendezvous bug. Insert **after `:46-47`** (the `home` / `csActiveDir` declarations) instead.

Selection rule: enumerate `$XDG_STATE_HOME/codescout/servers/*.json` (falling back to `~/.local/state/…`, matching the Rust side); stamp an entry when `entry.ppid` is on **this process's own ancestry**. Verified live 2026-08-18: the server's `ppid` is the `claude` process pid, and `claude` is also the hook's parent, so the hook's ancestry chain contains it.

Everything here is best-effort: every failure path is swallowed, and none may change the hook's exit code or its emitted context. A broken rendezvous costs the `/clear` fix, not the session.

- [ ] **Step 1: Write the failing test**

In `hooks/session-start.test.sh`, after the existing `ctx()` definition:

```bash
# --- rendezvous: the hook stamps slots whose ppid is on our ancestry ---
RV="$TMP/state/codescout/servers"; mkdir -p "$RV"
MYPPID=$(ps -o ppid= -p $$ | tr -d ' ')
printf '{"pid":999001,"ppid":%s,"started_at":"2026-01-01T00:00:00Z","cwd":"/","session":null,"hook_at":null}' "$MYPPID" > "$RV/999001.json"
printf '{"pid":999002,"ppid":1,"started_at":"2026-01-01T00:00:00Z","cwd":"/","session":null,"hook_at":null}' > "$RV/999002.json"

XDG_STATE_HOME="$TMP/state" ctx startup >/dev/null

jq -e '.session == "sst-startup" and .hook_at != null' "$RV/999001.json" >/dev/null \
  && pass "rendezvous: entry on our ancestry is stamped" \
  || fail "rendezvous: entry on our ancestry was NOT stamped"

jq -e '.session == null and .hook_at == null' "$RV/999002.json" >/dev/null \
  && pass "rendezvous: unrelated entry is left alone" \
  || fail "rendezvous: unrelated entry was stamped — selection is too broad"
```

The second assertion is the important one: a rule that stamps everything passes a single-window test and corrupts every concurrent window in reality.

`ctx()` must be extended to pass `XDG_STATE_HOME` through to the hook process:

```bash
ctx() {
  printf '{"cwd":"%s","source":"%s","session_id":"sst-%s"}' "$TMP" "$1" "$1" \
    | XDG_STATE_HOME="${XDG_STATE_HOME-}" node "$HOOK" 2>/dev/null \
    | jq -r '.hookSpecificOutput.additionalContext // ""'
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `bash hooks/session-start.test.sh`
Expected: the two new assertions FAIL; every pre-existing assertion still PASSES. If a pre-existing one breaks, stop — `ctx()` was changed incorrectly.

- [ ] **Step 3: Implement the stamp block**

Insert after `:46-47` (see the placement note above — **not** after `:37`):

```js
// Stamp codescout rendezvous slots belonging to our own process ancestry.
// The server publishes <pid>.json at construction (it cannot learn the session
// id at startup: MCP initialize runs before SessionStart), and this writes the
// id into it. Matching on ppid-within-our-ancestry is what keeps two concurrent
// windows on one repo from stamping each other's servers.
if (sessionId) {
  try {
    const stateHome = process.env.XDG_STATE_HOME || join(home, '.local', 'state');
    const rvDir = join(stateHome, 'codescout', 'servers');
    if (existsSync(rvDir)) {
      const ancestry = ownAncestry();
      const stampedAt = new Date().toISOString();
      for (const name of readdirSync(rvDir)) {
        if (!name.endsWith('.json')) continue;
        const f = join(rvDir, name);
        try {
          const e = JSON.parse(readFileSync(f, 'utf8'));
          if (!ancestry.has(e.ppid)) continue;
          // Already current: rewriting would bump mtime and cost the server a
          // parse on its next call for no change.
          if (e.session === sessionId && e.hook_at) continue;
          e.session = sessionId;
          e.hook_at = stampedAt;
          writeFileSync(f, JSON.stringify(e));
        } catch {
          /* skip unreadable, unparseable, or concurrently-removed slots */
        }
      }
    }
  } catch {
    /* best-effort — never let the rendezvous break the hook */
  }
}
```

And the ancestry helper, placed beside it:

```js
// Our own pid chain, capped at 10 hops: a corrupt or cyclic chain must not spin
// inside a SessionStart hook, which blocks the session from starting.
function ownAncestry() {
  const seen = new Set();
  let pid = process.pid;
  for (let hop = 0; hop < 10; hop++) {
    if (pid <= 1 || seen.has(pid)) break;
    seen.add(pid);
    const parent = parentOf(pid);
    if (parent === null) break;
    pid = parent;
  }
  return seen;
}

function parentOf(pid) {
  try {
    if (process.platform === 'linux') {
      // Field 4 of /proc/<pid>/stat. The comm field (2) may contain spaces and
      // parens, so split after the LAST ')' rather than on whitespace.
      const stat = readFileSync(`/proc/${pid}/stat`, 'utf8');
      const rest = stat.slice(stat.lastIndexOf(')') + 2).split(' ');
      const ppid = Number.parseInt(rest[1], 10);
      return Number.isNaN(ppid) ? null : ppid;
    }
    const out = execFileSync('ps', ['-o', 'ppid=', '-p', String(pid)], {
      encoding: 'utf8',
      timeout: 2000,
    });
    const ppid = Number.parseInt(out.trim(), 10);
    return Number.isNaN(ppid) ? null : ppid;
  } catch {
    return null;
  }
}
```

`execFileSync` is not currently imported in `session-start.mjs` — add it to the existing `node:child_process` import beside `spawn`. (`lib.mjs` imports it separately for `git()`; that import is not in scope here.)

The `lastIndexOf(')')` detail is not pedantry: a process whose name contains a space or a paren — which `claude` and `node` wrappers routinely produce — shifts every whitespace-split field and silently yields the wrong ppid, so the hook would stamp nothing while appearing to work.

- [ ] **Step 4: Run to verify it passes**

Run: `bash hooks/session-start.test.sh`
Expected: all assertions PASS, including every pre-existing one.

- [ ] **Step 5: Verify end-to-end against a real server**

This is the only step that proves the two repos agree on the shape — neither repo's own tests can, since each mocks the other's side. With the codescout side built (`cargo rb`) and reconnected (`/mcp`):

1. Confirm `~/.local/state/codescout/servers/<pid>.json` exists for the live server, with `session` and `hook_at` populated.
2. Run `/clear`, then make any tool call.
3. Confirm `session` in that same file changed, and that the session-opening guide was re-emitted.

**Paste the observed file contents into the task report, before and after.** A report asserting this worked without the two file dumps will be rejected at review.

- [ ] **Step 6: Commit IN THE COMPANION REPO**

```bash
git -C /home/marius/work/claude/claude-plugins/codescout-companion commit -m "feat(session-start): stamp codescout rendezvous slots with the session id

MCP initialize runs before SessionStart, so the server publishes a slot and
this hook writes into it. Selection is by ppid-on-our-ancestry, so concurrent
windows do not cross-stamp." -- hooks/session-start.mjs hooks/session-start.test.sh
```
---

### Task 7: Documentation, memory, and bug closure

**Files:**
- Modify: `src/prompts/guides/workspace-state.md`, `docs/issues/2026-08-18-clear-leaves-mcp-session-id-stale.md`, memory `claude-code-mcp-env`

**Interfaces:** consumes everything; produces nothing.

**Context an implementer cannot infer:** `workspace-state.md` is `include_str!`'d at `src/prompts/mod.rs:442` and hard-injected into every session, so a stale sentence there ships a falsehood in the guide whose job is to describe this exact machinery. Three whole-corpus invariants iterate every guide body — `guide_topics_have_bodies`, `guide_bodies_contain_no_deprecated_tool_names`, `no_guide_claims_a_move_preserves_the_id` — so run `cargo test --lib prompts` after editing. **Do not touch the activation-semantics sentence at `:51`**: it describes the clear-on-every-activate behaviour Phase C replaces, and it is still true today.

`ONBOARDING_VERSION` does **not** need bumping — a guide body is not one of the three prompt surfaces.

- [ ] **Step 1: Update the guide.** Describe the two tiers and that the ledger now follows the conversation rather than the project. Storage/identity sentences only.
- [ ] **Step 2: Update memory `claude-code-mcp-env`** with the lifecycle matrix from the spec's "Lifecycle matrix (measured 2026-08-18)" section.
- [ ] **Step 3: Close the bug file.** In `docs/issues/2026-08-18-clear-leaves-mcp-session-id-stale.md`, set `status: fixed`, fill `closed:`, name the regression tests (`session_change_rearms_everything` plus the two hook assertions), and record the fix SHA labelled `experiments`. Run `git rev-list --left-right --count master...experiments` first: a `0` on the left means fast-forward is available, in which case do **not** write a pending-master-SHA Resume line — the experiments SHA already is the master SHA.
- [ ] **Step 4: Archive it through the librarian** — `artifact(action="update", …, patch={status:"fixed"})` then `artifact(action="move", …, new_rel_path="docs/issues/archive/…")`. Never a bare `git mv`: `id = sha256(abs_path)`, so a hand-move orphans the catalog row's events and augmentation.
- [ ] **Step 5: Run the full gate and commit.**

---

## Verification before reporting Phase B complete

1. `cargo fmt && cargo clippy -- -D warnings && cargo test` — green in codescout.
2. `bash hooks/session-start.test.sh` — green in the companion repo.
3. **On the wire, not in tests:** `cargo rb`, `/mcp`, then confirm a `servers/<pid>.json` exists for the live server with `hook_at` set; run `/clear` and confirm `session` changes in that file and the next tool call re-emits the session opener. Steps 1 and 2 cannot catch a cross-repo shape disagreement; only this can.
4. Both repos committed separately, each with a pathspec, and `git status --short` in each shows no swept foreign files.

## Deliberately NOT in this plan

- **§4 re-arm predicate and §5 opener predicate** — Phase C. They consume `is_active()` from Task 5.
- **`_meta` rank 3** — Ruling 1.
- **`workspace-state.md:51`'s activation sentence** — still true until Phase C.
- **Subagent cardinality and `--fork-session` context carryover** — spec "Out of scope". Both are real and tracked separately.
