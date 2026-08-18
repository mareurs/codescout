# Guide Ledger Phase C — Re-arm and Opener Predicates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop `workspace(action="activate")` wiping the whole guide ledger. Re-arm only the project-scoped topic, only on a genuine project switch, and only while conversation changes are detectable.

**Architecture:** Two predicates. The **re-arm predicate** (§4) moves the ledger reset out of `ActivateProject::call`'s first line into the full-activation path, fires it only when the workspace root actually changed, and gates the precise behaviour on the Phase B rendezvous being active — so a user without the companion plugin keeps today's blunt behaviour and cannot regress. The **opener predicate** (§5) changes the session-opener trigger from "the ledger is empty" to "the ledger lacks the bootstrap topic", which is what makes a surgical re-arm observable at all.

**Tech Stack:** Rust (rmcp 1.3, parking_lot, tokio).

**Spec:** `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` — §4 re-arm predicate, §5 opener predicate. **Read those two sections, and read the Corrections block below before trusting any line number in them.**

**Predecessors:** Phase A (storage, stamped shape, `re_arm`/`expire_idle`, GC) and Phase B (`SessionKey`, anonymous-tier TTL, the pid-keyed rendezvous, `poll`/`is_active`/`rekey`, and the companion hook). Both merged. **This phase is the payload** — it removes the ~900K-token waste the spec was opened for.

## Corrections to the spec — verified at the bytes 2026-08-18

The spec was authored before Phases A and B landed. A scout re-measured every claim §4 and §5 depend on. **Trust this block over the spec.**

| Spec says | Reality | Consequence if trusted |
|---|---|---|
| `ctx.session_key.rendezvous_active()` | **No such method on that type.** `SessionKey` (`src/tools/session_key.rs:17-22`) is a bare enum with only `id()`. The method is `Rendezvous::is_active()` (`src/tools/rendezvous.rs:108-110`), on a different struct that is **not** on `ToolContext`. | Does not compile; the design's central gate has no plumbing. See Ruling 1. |
| `activate_project_resets_hints` at `src/server.rs:4292-4318` | At **`4724-4750`** — off by 432 lines. | Edits the wrong function. |
| `make_server()` at `src/server.rs:3822` | Does not exist. Two exist: `tests::make_server` at `1910`, and the one this test uses, `guide_hint_tests::make_server`, at **`4217-4239`**. | Edits the wrong helper. |
| Opener check at `src/tools/core/types.rs:692` / `:693` (cited twice, inconsistently) | At **`:703`**. `:691` is the lock acquisition. | Edits the lock line. |
| `Agent::activate:528` carries the canonicalization comment | Call is at **`agent/mod.rs:547`**, comment from `543`. | Cites a line that does not say what is claimed. |
| "Both sides must be canonicalized" | **Already true.** `Agent::activate` canonicalizes before storing (`agent/mod.rs:547`), and `config/mod.rs:198` canonicalizes the incoming root. | Adds a redundant `canonicalize()` call. |
| Implies `re_arm` must be built | **Already exists** — `GuideLedger::re_arm` (`src/tools/guide_ledger.rs:231-241`), built in Phase B, matching the sketch exactly. | Reimplements existing API. |
| `PROJECT_SCOPED` | Does not exist anywhere in `src/**`. A sketch name, not a real constant. | — (this plan creates it) |

## Global Constraints

- **Degrade to re-sending, never to suppressing.** The governing invariant. A path that re-sends a guide the model already holds costs tokens once; a path that withholds one is silent, and is the defect this whole effort exists to remove. **Any change that could withhold a guide is wrong even if it is more efficient.**
- **The precise path is gated, and the gate is the safety mechanism — not a procedural nicety.** If the rendezvous has never reported in, a `/clear` is invisible, and precise re-arming would silently starve the new conversation. Gated: a Claude Code user *with* the companion gets the full saving; one *without* it keeps today's behaviour exactly; tier-2 clients never satisfy the gate. **No regression is possible for anyone.**
- **Agent-Agnostic Design.** No branching on `clientInfo.name`, no assuming Claude Code.
- **Tests must never call `std::env::set_var`** — UB under concurrent readers. Inject through `ServerEnv`.
- **No test may read, write, or GC the developer's real per-user state directory.** Inject `guide_hints_dir` and `servers_dir`; four `test_env` helpers exist (`src/server.rs` ×2, `src/peer/client.rs`, `src/peer/server.rs`).
- **`cargo fmt`, `cargo clippy -- -D warnings` (default AND `--features librarian,dashboard`), and the focused suites must pass.** **Do not run the full `cargo test`** — a concurrent session works in this checkout and a full run reads its mid-edit tree.
- **Commit with an explicit pathspec.** Before committing a file the concurrent session also edits (`src/server.rs` especially), run `git diff -U0 <file>` bare and confirm every hunk is yours; if not, stop and report BLOCKED. Pathspec discipline does not protect a same-file collision.

## Rulings made before execution

1. **The gate lives on `GuideLedger`, not on `ToolContext`.** The spec's sketch needs `is_active()` inside `ActivateProject::call`, but `Rendezvous` is on `CodeScoutServer` and polled in `call_tool_inner` (a Phase B decision, taken because `ToolContext` has **126 construction sites across 21 files**). Measured alternatives: threading a new `ToolContext` field touches all 126; storing the flag on `GuideLedger` touches **0**, because `ToolContext` already carries `guide_hints_emitted`. It also costs no extra lock — `ActivateProject::call` already locks that mutex for the reset — and keeps the gate beside the state it gates. *Cost if wrong:* `GuideLedger` carries one rendezvous-shaped bool that is arguably not its data. A cohesion tax against a 126-site edit surface.
2. **The startup-time gap is closed by re-arming on any non-empty loaded ledger, not by persisting a root.** The spec requires the predicate to fire on a startup project difference. There is **no stored value anywhere** to compare against: `persist` serializes only `emitted`, and the `Rendezvous` `Entry`'s `cwd` describes the current server, not the session's history. Persisting a root means a third on-disk shape plus migration from both predecessors. Instead: if the ledger loaded non-empty, re-arm the project-scoped topic. *Cost:* exactly one bootstrap re-injection per `/mcp` reconnect within a conversation (~1 of ~10 guide bodies) — and it errs toward re-sending. Confirmed with the user 2026-08-18.
3. **`ProjectStatus::call`'s `post_compact` clear stays untouched** (`src/tools/config/mod.rs:278`). Compaction genuinely summarises every guide body out of context, so a full wipe is correct there. It is a different trigger with different semantics and the spec's testing table marks it unchanged.
4. **Task order is load-bearing: the opener predicate ships before the re-arm predicate.** Removing one topic from a set of ten leaves the set non-empty, so under today's `is_empty()` trigger a surgical re-arm would inject **nothing**. Landing §4 first would ship a change that silently does nothing. *Cost if wrong:* none — this only constrains sequencing.

5. **Server-level test bodies in Tasks 3 and 4 are specified as intent plus the exact model test to copy, not as verbatim code — deliberately.** The writing-plans rubric wants complete code, and Tasks 1 and 2 supply it. But the predecessor phase shipped verbatim test code that was **wrong four times** (a missing import, a mutation-kill claim the test could not achieve, an assertion probing a field the tool's output form discards, a fixture using a pid that reads as alive). Each cost a round trip. These tests build servers through helpers whose exact signatures shift with the file, and the implementer will have that file open while I do not. Naming the property, the mutation that must kill it, and the sibling test to pattern-match is more reliable here than confident code I cannot compile. *Cost if wrong:* an implementer writes a differently-shaped test than imagined; the mutation requirements in each step are what actually constrain it, and those are exact.
## File Structure

**Modified:**

- `src/tools/guide_ledger.rs` — the `rendezvous_active` flag, its setter and getter.
- `src/server.rs` — `poll_rendezvous` sets the flag; `from_parts_with_env` gains the startup re-arm; `guide_hint_tests` gains and splits tests.
- `src/tools/core/types.rs` — the opener predicate, one line at `:703`.
- `src/tools/config/mod.rs` — `PROJECT_SCOPED`, and the reset moves from `:139` into the full-activation path after `:198`.
- `src/prompts/guides/workspace-state.md` — the activation-semantics sentence, which this phase finally makes false.

**No new files.** Every mechanism this phase needs already exists; Phase C is predicates and placement.

---

### Task 1: The rendezvous gate, readable from a tool

**Files:**
- Modify: `src/tools/guide_ledger.rs`, `src/server.rs` (`poll_rendezvous`)
- Test: `src/tools/guide_ledger.rs` (inline `mod tests`), `src/server.rs` (`guide_hint_tests`)

**Interfaces:**
- Consumes: `Rendezvous::is_active()` (`src/tools/rendezvous.rs:108-110`) — **zero production callers today**; this task is its first.
- Produces: `GuideLedger::set_rendezvous_active(&mut self, active: bool)` and `GuideLedger::rendezvous_active(&self) -> bool`. Task 3's predicate gates on the getter.

**Context an implementer cannot infer:** `Rendezvous.active` is set inside `poll()` — `if entry.hook_at.is_some() { self.active = true; }` — i.e. only once the companion hook has stamped the slot. It is **latching**: once true it stays true for the process. `CodeScoutServer::poll_rendezvous` (`src/server.rs:832-843`) already calls `self.rendezvous.lock().poll()` on every request and branches on the returned `Option<String>`; it never reads `is_active()`. You are adding that read.

Do **not** thread `Rendezvous` onto `ToolContext` — see Ruling 1. The flag is copied into the ledger, which every tool already has.

- [ ] **Step 1: Write the failing tests**

In `src/tools/guide_ledger.rs`'s `mod tests`:

```rust
    #[test]
    fn a_fresh_ledger_reports_no_rendezvous() {
        // The gate must default CLOSED. A ledger that reports an active
        // rendezvous it never had would let Task 3 take the precise path on a
        // client where conversation changes are invisible — silently starving
        // the new conversation, which is the one thing the invariant forbids.
        let l = GuideLedger::anonymous(None);
        assert!(!l.rendezvous_active());
    }

    #[test]
    fn the_rendezvous_flag_round_trips() {
        let mut l = GuideLedger::anonymous(None);
        l.set_rendezvous_active(true);
        assert!(l.rendezvous_active());
    }

    #[test]
    fn rekey_preserves_the_rendezvous_flag() {
        // A session change means a NEW conversation, not a new server. The hook
        // that stamped our slot is still installed, so the gate must stay open
        // across a rekey — closing it would drop the whole feature the first
        // time /clear fires, which is exactly when it is needed.
        let mut l = GuideLedger::anonymous(None);
        l.set_rendezvous_active(true);
        l.rekey("conv-B");
        assert!(l.rendezvous_active(), "rekey must not close the gate");
    }

    #[test]
    fn clear_preserves_the_rendezvous_flag() {
        // Same reasoning for the blunt path: clearing what the ledger holds
        // says nothing about whether a hook is installed.
        let mut l = GuideLedger::anonymous(None);
        l.set_rendezvous_active(true);
        l.insert("librarian".to_string());
        l.clear();
        assert!(l.rendezvous_active(), "clear must not close the gate");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib guide_ledger`
Expected: compile failure — `no method named 'rendezvous_active'` / `'set_rendezvous_active'`.

- [ ] **Step 3: Implement**

Add the field to `GuideLedger` beside `idle_ttl`:

```rust
    /// Has a companion hook ever stamped this server's rendezvous slot?
    ///
    /// Copied in from `Rendezvous::is_active()` by `poll_rendezvous` on every
    /// request. It lives here rather than on `ToolContext` because the ledger is
    /// what it gates and is already reachable from every tool, where
    /// `Rendezvous` is not (126 `ToolContext` construction sites).
    ///
    /// Latching: once a hook has reported in, it has reported in. Survives
    /// `clear` and `rekey`, neither of which says anything about whether a hook
    /// is installed.
    rendezvous_active: bool,
```

Every constructor must initialise it `false` — `load`, `anonymous`, `mid_session`, and the derived `Default`. **`clear` and `rekey` must not reset it**; both currently assign or clear individual fields rather than replacing `self`, so verify by reading them rather than assuming.

```rust
    /// Record whether the rendezvous has reported in. See the field's docs.
    pub fn set_rendezvous_active(&mut self, active: bool) {
        self.rendezvous_active = active;
    }

    /// Is it safe to re-arm surgically rather than bluntly? See the field's docs.
    pub fn rendezvous_active(&self) -> bool {
        self.rendezvous_active
    }
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --lib guide_ledger`
Expected: PASS, including all pre-existing tests.

- [ ] **Step 5: Wire it in `poll_rendezvous`**

In `src/server.rs:832-843`, copy the flag across on every poll. The rendezvous lock is a statement temporary — **do not hold it while taking the ledger lock**; Phase B's review verified that ordering and it must survive:

```rust
        let (changed, active) = {
            let mut rv = self.rendezvous.lock();
            (rv.poll(), rv.is_active())
        };
        let mut led = self.guide_hints_emitted.lock();
        led.set_rendezvous_active(active);
        if let Some(new_session) = changed {
            led.rekey(&new_session);
        }
```

Note this also retires the `#[expect(dead_code)]` situation around `Rendezvous`: `is_active()` now has a production caller. If clippy reports an unfulfilled expectation anywhere as a result, remove that attribute in this commit — that is the attribute working as designed.

- [ ] **Step 6: Add a server-level test**

In `guide_hint_tests`, prove the flag reaches the ledger through a real request — the wiring, not the unit. Follow `a_tool_call_polls_the_rendezvous_and_re_arms` (`src/server.rs`, in the same module) for the shape: build a server with an injected `servers_dir`, stamp its slot with a `hook_at`, drive one tool call, then assert `server.guide_hints_emitted.lock().rendezvous_active()` is true. Assert it is **false** before the stamp.

**Two tasks in the predecessor shipped tests whose comments claimed mutation-kills they did not achieve.** Verify this one by applying the mutation: delete the `led.set_rendezvous_active(active);` line, confirm the test goes RED, revert, confirm GREEN. Paste the observed output into your report.

- [ ] **Step 7: Gate and commit**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo clippy --features librarian,dashboard -- -D warnings && cargo test --lib guide_ledger && cargo test --lib guide_hint_tests`

```bash
git commit -m "feat(guide-ledger): carry the rendezvous gate on the ledger it gates

Rendezvous::is_active gets its first production caller. The flag rides on
GuideLedger rather than ToolContext because the ledger is already reachable
from every tool and ToolContext has 126 construction sites." -- src/tools/guide_ledger.rs src/server.rs
```

---

### Task 2: The opener predicate

**Files:**
- Modify: `src/tools/core/types.rs` (one line at `:703`)
- Test: `src/server.rs` (`guide_hint_tests`)

**Interfaces:**
- Consumes: `crate::prompts::SESSION_OPENING_GUIDE` (`src/prompts/mod.rs:429`, value `"project-activation-bootstrap"`), and `GuideLedger::contains` (`src/tools/guide_ledger.rs:132-134`).
- Produces: nothing new. **Task 3 depends on this landing first** — see below.

**Context an implementer cannot infer — and why this task is not cosmetic:**

`Tool::call_content` fires the session opener on `emitted.is_empty()` at **`src/tools/core/types.rs:703`** (the lock is at `:691`; the spec's `:692`/`:693` citations are stale). Since `SESSION_OPENING_GUIDE` **is** `"project-activation-bootstrap"`, removing that one topic from a set still holding nine others leaves the set non-empty — so Task 3's surgical `re_arm` would inject **nothing at all**. That is why Ruling 4 orders this task first: shipping Task 3 against an `is_empty()` trigger would be a change that silently does nothing.

It also retires a latent bug that exists today, independent of this phase: if a session's first codescout call is an explicit `get_guide("librarian")`, that insert makes the set non-empty and the session opener is suppressed **for the rest of the session**.

`Workspace::relevant_guide_topic` (`src/tools/config/mod.rs:91-107`) returns `Some("workspace-state")` unconditionally — **not** the bootstrap. Its own comment records that it used to return `SESSION_OPENING_GUIDE` and that this was redundant precisely because `call_content`'s branch fires the opener regardless. So `call_content` is the only place the opener can come from; there is no second path to update.

- [ ] **Step 1: Write the failing tests**

In `guide_hint_tests` (`src/server.rs`). Both tests must start from a **non-empty** ledger — that is the whole point, and it is what every existing opener test fails to do (all four build a fresh server, where `is_empty()` and `!contains(...)` agree trivially).

```rust
    #[tokio::test]
    /// The §5 predicate. A ledger holding other topics but NOT the bootstrap
    /// must still fire the opener. Under the old `is_empty()` trigger this is
    /// false — which is why a surgical re-arm would inject nothing.
    async fn opener_fires_when_bootstrap_absent_from_a_nonempty_set() {
        let (dir, server) = make_server().await;
        let ctx = shared_ctx(&server);

        // Seed a non-empty ledger that deliberately lacks the bootstrap topic.
        {
            let mut led = server.guide_hints_emitted.lock();
            led.insert("librarian".to_string());
            led.insert("progressive-disclosure".to_string());
            assert!(!led.is_empty());
            assert!(!led.contains(crate::prompts::SESSION_OPENING_GUIDE));
        }

        let tool = tool_by_name(&server, "run_command");
        let result = tool
            .call_content(json!({"command": "echo hi"}), &ctx)
            .await
            .unwrap();

        assert!(
            content_carries_guide_body(&result, crate::prompts::SESSION_OPENING_GUIDE),
            "a ledger without the bootstrap topic must re-open the session, \
             even though it is not empty"
        );
        let _ = dir;
    }

    #[tokio::test]
    /// Retires a latent bug: an explicit get_guide as the session's first call
    /// made the set non-empty and suppressed the opener for the whole session.
    async fn explicit_get_guide_first_does_not_suppress_the_opener() {
        let (dir, server) = make_server().await;
        let ctx = shared_ctx(&server);

        let guide = tool_by_name(&server, "get_guide");
        let _ = guide
            .call_content(json!({"topic": "librarian"}), &ctx)
            .await
            .unwrap();

        let tool = tool_by_name(&server, "run_command");
        let result = tool
            .call_content(json!({"command": "echo hi"}), &ctx)
            .await
            .unwrap();

        assert!(
            content_carries_guide_body(&result, crate::prompts::SESSION_OPENING_GUIDE),
            "an explicit get_guide must not consume the session opener"
        );
        let _ = dir;
    }
```

`content_carries_guide_body` may not exist. If not, write it as a small helper in the same module that scans the returned `Vec<Content>` for a text block containing the topic's auto-injection marker. **Do not use `extract_hint`** — it parses block 0 as JSON, and a text-form tool's primary block is not JSON, so it returns `None` regardless. That trap cost a round in the predecessor.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib guide_hint_tests`
Expected: both new tests FAIL — the ledger is non-empty, so the old predicate suppresses the opener. Paste the failure output; it is the evidence that these tests discriminate.

- [ ] **Step 3: Implement**

`src/tools/core/types.rs:703`:

```rust
-        if emitted.is_empty() {
+        if !emitted.contains(crate::prompts::SESSION_OPENING_GUIDE) {
```

Update the comment above it. It currently explains the empty-ledger reasoning; it must now say the opener fires whenever the bootstrap topic specifically is absent, and why that is the right trigger (a surgical re-arm removes exactly that topic and must be observable).

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --lib guide_hint_tests`
Expected: PASS, **including the four pre-existing opener tests** — `session_opener_fires_once_not_on_every_call` (`:4515`), `session_opener_defers_but_does_not_consume_the_tools_topic` (`:4539`), `session_opens_with_bootstrap_from_a_non_workspace_tool` (`:4483`), `a_refusal_does_not_suppress_the_session_opening_guide` (`:4357`). All four build a fresh server, so both predicates agree for them; if any breaks, the change did more than intended.

- [ ] **Step 5: Gate and commit**

```bash
git commit -m "feat(guide-ledger): open the session on a missing bootstrap, not an empty set

Removing one topic from a set of ten leaves it non-empty, so a surgical
re-arm would have injected nothing. Also retires a latent bug where an
explicit get_guide first call suppressed the opener for the whole session." -- src/tools/core/types.rs src/server.rs
```

---

### Task 3: The re-arm predicate

**Files:**
- Modify: `src/tools/config/mod.rs` (`PROJECT_SCOPED`; move the reset off `:139` into the full-activation path after `:198`)
- Test: `src/server.rs` (`guide_hint_tests` — including splitting one existing test)

**Interfaces:**
- Consumes: `GuideLedger::rendezvous_active()` (Task 1), `GuideLedger::re_arm` (`src/tools/guide_ledger.rs:231-241`, already exists), `AgentInner::default_workspace_root` (`src/agent/mod.rs:106`).
- Produces: `PROJECT_SCOPED`.

**Context an implementer cannot infer — read all of it:**

`ActivateProject::call` spans `src/tools/config/mod.rs:138-235`. The unconditional `ctx.guide_hints_emitted.lock().clear();` is the **first statement of the function**, at `:139`. Because it is first, it fires on *every* call — including a **missing or non-string `path`** (the param check is at `:141`), a **nonexistent directory** (checked at `:191`), and the **bare-project-id focus-switch path** (`:151-186`, which returns early at `:185`). Moving the reset into the full-activation path after root resolution therefore fixes three things at once, and two of them are named tests in the spec's table.

Root resolution and canonicalization complete at **`:198`** (`let root = root.canonicalize().unwrap_or(root);`). The focus-switch path never reaches that line.

**The comparand is `AgentInner::default_workspace_root`** (`src/agent/mod.rs:106`, `pub`, behind `pub inner: Arc<tokio::sync::RwLock<AgentInner>>`), **not** `Agent::project_root()` — the latter is `focused_project_root()` (`src/agent/mod.rs:1382-1385`), so with focus on a sub-project like `crates/codescout-embed` a re-activation of the repo root reads as a project change. That is friction F-52.

**Both sides are already canonical.** `Agent::activate` canonicalizes before storing (`src/agent/mod.rs:547`, with a doc comment at `:543` recording the bug that motivated it), and `:198` canonicalizes the incoming root. The spec's instruction to canonicalize both sides is already satisfied — **do not add a redundant `canonicalize()` call.**

- [ ] **Step 1: Split the test that encodes the old policy**

`activate_project_resets_hints` (`src/server.rs:4724-4750`) must be **split, not tweaked**. It activates `dir.path()` — the *same* root `guide_hint_tests::make_server` (`:4217-4239`) built the agent with — and asserts the tool-contract topic `"librarian"` re-emits afterwards. Both halves are tied to the old semantics: it is a same-project re-activation (so the new predicate correctly does **not** re-arm), and `"librarian"` is not project-scoped (so even a genuine switch would not re-arm it).

Replace it with two tests that each assert one thing:

```rust
    #[tokio::test]
    /// Re-activating the SAME project must leave the ledger alone. This is the
    /// saving: today every activate wipes ~10 guide bodies out of the ledger and
    /// they all re-inject on the next call.
    async fn activate_same_project_keeps_hints() {
        // build server, emit a topic, activate the SAME root make_server used,
        // assert the topic is still present afterwards.
    }

    #[tokio::test]
    /// A genuine project switch re-arms the project-scoped topic and NOTHING
    /// else — the tool-contract guides the model still holds must survive.
    async fn activate_different_project_rearms_bootstrap_only() {
        // build server, emit bootstrap + librarian, activate a DIFFERENT tempdir,
        // assert bootstrap is gone and librarian remains.
    }
```

The second assertion in `activate_different_project_rearms_bootstrap_only` is the one that matters: a mutation replacing `re_arm(PROJECT_SCOPED)` with `clear()` passes the first assertion and fails only the second.

Both need the rendezvous gate **open** (Task 1) or they will take the blunt path. Stamp the slot as `a_tool_call_polls_the_rendezvous_and_re_arms` does, or set the flag directly on the ledger.

- [ ] **Step 2: Write the remaining failing tests**

```rust
    #[tokio::test]
    /// The bare-project-id focus switch returns early via
    /// activate_within_workspace and must not touch the ledger at all.
    async fn subproject_focus_switch_does_not_rearm() { }

    #[tokio::test]
    /// A missing or malformed `path` must not wipe the ledger. Today it does,
    /// because the clear is the function's first statement and the param check
    /// is two lines later.
    async fn malformed_activate_leaves_ledger_intact() { }

    #[tokio::test]
    /// Without a rendezvous a /clear is invisible, so the precise path would
    /// starve the new conversation. The gate must fall back to the blunt clear.
    async fn without_a_rendezvous_activate_still_clears_everything() { }
```

That third test is the safety property, and it is the one a careless implementation loses: an implementation that always takes the precise path passes every other test here.

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test --lib guide_hint_tests`
Expected: the new tests fail against today's unconditional clear. Record which fail and why.

- [ ] **Step 4: Implement**

Add near the other constants in `src/tools/config/mod.rs`:

```rust
/// Topics forgotten on a genuine project switch. Deliberately just the one:
/// the tool-contract guides the model already holds stay valid across a switch,
/// and re-sending them is the waste this phase exists to remove.
const PROJECT_SCOPED: &[&str] = &["project-activation-bootstrap"];
```

Delete the `clear()` at `:139`. After `:198`, inside the full-activation path only:

```rust
        // Re-arm BEFORE the activation mutates `default_workspace_root`, or the
        // comparison always reads "same project".
        let switched = {
            let inner = ctx.agent.inner.read().await;
            inner.default_workspace_root.as_deref() != Some(root.as_path())
        };
        {
            let mut led = ctx.guide_hints_emitted.lock();
            if led.rendezvous_active() {
                if switched {
                    led.re_arm(PROJECT_SCOPED);
                }
            } else {
                // No rendezvous ⇒ a /clear is invisible to us ⇒ precise re-arming
                // could starve the new conversation. Keep the blunt behaviour.
                led.clear();
            }
        }
```

**Ordering is load-bearing and is not in the spec:** the read of `default_workspace_root` must happen *before* whatever call sets it to the new root, or `switched` is always false and the whole feature is inert. Read the surrounding code and place it accordingly; if activation happens before `:198`, put the comparison earlier and carry the boolean.

**Do not hold the `agent.inner` read guard across the `guide_hints_emitted` lock.** The block above drops it at the `;`. Phase B's review verified an equivalent ordering in `poll_rendezvous`; preserve that discipline.

- [ ] **Step 5: Run to verify they pass, then prove the gate**

Run: `cargo test --lib guide_hint_tests`

Then apply these mutations and record the observed RED/GREEN for each:
1. `re_arm(PROJECT_SCOPED)` → `clear()` — `activate_different_project_rearms_bootstrap_only`'s second assertion must go RED.
2. Remove the `rendezvous_active()` gate so the precise path always runs — `without_a_rendezvous_activate_still_clears_everything` must go RED.
3. Invert `switched` — `activate_same_project_keeps_hints` must go RED.

A mutation that stays GREEN is a coverage gap you must close before reporting.

- [ ] **Step 6: Gate and commit**

```bash
git commit -m "feat(guide-ledger): re-arm only the project topic, only on a real switch

Moves the reset out of ActivateProject::call's first line, so a malformed
path and a sub-project focus switch stop wiping the ledger. Gated on the
rendezvous: without one, the blunt clear stays, so no user can regress." -- src/tools/config/mod.rs src/server.rs
```

---

### Task 4: Close the startup-time suppression

**Files:**
- Modify: `src/server.rs` (`from_parts_with_env`)
- Test: `src/server.rs` (`guide_hint_tests`)

**Interfaces:**
- Consumes: `GuideLedger::re_arm` and `is_empty` (both existing), and `PROJECT_SCOPED` from Task 3 — which lives in `src/tools/config/mod.rs`. Either make it `pub(crate)` and import it, or define the array inline here with a comment pointing at the other; **do not let the two lists drift.**
- Produces: nothing consumed downstream.

**Context an implementer cannot infer — this closes a debt two phases old:**

Phase A made the ledger key on **session alone**, where it had previously been implicitly *(session, project)*. That is what lets a ledger survive a project switch, and it was the point. But it created a narrow suppression, which Phase A's whole-branch review found and deliberately recorded rather than patched — because any Phase A fix meant putting the project back into the key, undoing the change.

**The case:** one conversation, MCP server restarts against a **different** `--project`. Previously the new project's directory held no ledger for that session, so `emitted` loaded empty and the bootstrap guide fired for the new project. Now the session-keyed file is found, the previous project's topics are already in it, and `project-activation-bootstrap` is **suppressed** for the new project until something calls `workspace(activate)`.

That is the wrong direction under the governing invariant, and Task 3's predicate does **not** reach it: the suppression appears at server construction, before any `activate` has run.

**The fix, and why it is not a root comparison** (Ruling 2, confirmed with the user): there is **no stored value anywhere** to compare against. `GuideLedger::persist` (`src/tools/guide_ledger.rs:304-318`) serializes only `emitted`; `load` (`:84-98`) reconstructs only `path`/`emitted`/`notices`/`idle_ttl`. The `Rendezvous` `Entry`'s `cwd` (`src/tools/rendezvous.rs:24`) describes the *current* server at its own publish time, keyed by pid — not "the root this session was last seen with." Persisting one means a third on-disk shape plus migration from both the legacy `Vec<String>` and the current bare map.

Instead: **if the ledger loaded non-empty, re-arm the project-scoped topic.** A non-empty ledger at construction means a prior server already served this conversation, so this is either a reconnect against the same project (re-arm costs one bootstrap re-send) or against a different one (re-arm is exactly right). The cost is bounded and known: **one bootstrap injection per `/mcp` reconnect within a conversation**, roughly 1 of ~10 guide bodies, and it errs toward re-sending.

The ledger is loaded at `src/server.rs:391-392`. Note `guide_project_root` is already in scope from `:332` — **you do not need it**; resist using it, since it reads `project_root()` (the *focused* root), which is the wrong comparand per F-52 and would reintroduce exactly the bug Task 3 avoids.

- [ ] **Step 1: Write the failing tests**

```rust
    #[tokio::test]
    /// The Phase A debt. One session id, two server constructions with
    /// DIFFERENT roots: the second must re-open the session for its new
    /// project. Before this task the session-keyed ledger carries the first
    /// project's topics, `contains(SESSION_OPENING_GUIDE)` is true, and the
    /// bootstrap is suppressed for a project that never received it.
    async fn a_restart_against_a_different_project_reopens_the_session() {
        // dir_a, dir_b: two tempdirs. Same session_id_explicit, same
        // guide_hints_dir, so both constructions share one persisted ledger.
        // 1. Build against dir_a, drive a call, assert bootstrap present.
        // 2. Drop. Build against dir_b with the SAME session id.
        // 3. Assert the reloaded ledger does NOT contain the bootstrap topic,
        //    and that a call against the new server re-emits it.
    }

    #[tokio::test]
    /// The accepted cost of Ruling 2, pinned so it is a decision and not a
    /// surprise: a same-project reconnect also re-arms the bootstrap. One
    /// guide body re-sent per reconnect, deliberately, to avoid persisting a
    /// project root and a third on-disk ledger shape.
    async fn a_same_project_restart_also_rearms_the_bootstrap() {
        // Same as above but dir_a both times. Assert bootstrap absent after
        // reload, AND assert a tool-contract topic (e.g. "librarian") SURVIVED
        // — the re-arm must stay surgical even here.
    }
```

The second assertion in the second test is what keeps this honest: if the startup path used `clear()` instead of `re_arm`, Phase A's whole reason for existing — the ledger surviving `/mcp` restarts — would be silently undone, and the first test would not notice.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib guide_hint_tests`
Expected: `a_restart_against_a_different_project_reopens_the_session` FAILS — the reloaded ledger still contains the bootstrap. Paste the output.

- [ ] **Step 3: Implement**

Immediately after the ledger is constructed at `src/server.rs:391-392`:

```rust
        // A non-empty ledger at construction means a prior server already served
        // this conversation. Either this is a reconnect against the same project
        // (re-arming the bootstrap costs one re-send) or against a different one
        // (re-arming is exactly right, and closes the suppression Phase A
        // knowingly created when it dropped the project from the ledger key).
        //
        // Deliberately NOT a root comparison: nothing persists the root a session
        // was last seen with, and adding one means a third on-disk shape plus
        // migration from two predecessors. See the plan's Ruling 2.
        {
            let mut led = guide_hints_emitted.lock();
            if !led.is_empty() {
                led.re_arm(PROJECT_SCOPED);
            }
        }
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --lib guide_hint_tests` and `cargo test --lib guide_ledger`
Expected: PASS. **`guide_ledger_survives_mcp_restart` must still pass** — it asserts a tool-contract topic survives a reconnect, which this change preserves precisely because it re-arms rather than clears. If it breaks, you used `clear()`.

- [ ] **Step 5: Prove it by mutation**

Replace `re_arm(PROJECT_SCOPED)` with `clear()`. `a_same_project_restart_also_rearms_the_bootstrap`'s survival assertion and `guide_ledger_survives_mcp_restart` must both go RED. Revert; confirm GREEN. Then delete the whole block: the first test must go RED. Paste both observations.

- [ ] **Step 6: Gate and commit**

```bash
git commit -m "fix(guide-ledger): re-open the session when a reconnect changes project

Closes the suppression Phase A knowingly created by keying the ledger on
session alone. Re-arms the project topic whenever a ledger loads non-empty,
which costs one bootstrap re-send per reconnect and avoids persisting a root." -- src/server.rs
```

---

### Task 5: The guide, and the residuals

**Files:**
- Modify: `src/prompts/guides/workspace-state.md`, `docs/trackers/2026-08-18-guide-ledger-residuals.md`
- Test: `cargo test --lib prompts`

**Interfaces:** consumes everything; produces nothing.

**Context an implementer cannot infer:** `src/prompts/guides/workspace-state.md` is `include_str!`'d into the binary and hard-injected into **every session's context**, so a stale sentence there ships a falsehood in the guide whose job is to describe this exact machinery.

Its activation-semantics sentence (around `:51`) currently says the ledger is *"Cleared on every activation… After a clear, the next of either re-emits."* **Phases A and B were both told to leave that sentence alone because it was still true. This phase is what makes it false** — so it must now change, and this is the only task in the whole effort permitted to touch it.

It must now describe: a same-project re-activation keeps the ledger; a genuine project switch re-arms only the project-scoped topic; and — because the Agent-Agnostic contract is the user-visible part — that without the companion rendezvous the blunt clear-on-every-activate behaviour is retained deliberately, so nothing regresses.

Three whole-corpus invariants iterate every guide body. Find them by grepping for the test names in `src/prompts/mod.rs`, run `cargo test --lib prompts`, and report which you found and their results. **`ONBOARDING_VERSION` must NOT be bumped** — a guide body is not one of the three prompt surfaces.

- [ ] **Step 1: Update the guide.** Storage/identity sentences are already correct from Phase B; change the activation-semantics sentence only.
- [ ] **Step 2: Run `cargo test --lib prompts`.** Report the invariant names and results.
- [ ] **Step 3: Update the residuals tracker.** `docs/trackers/2026-08-18-guide-ledger-residuals.md` carries entries that Phase C closes. Read it, mark what this phase resolved, and leave the rest with their reasons intact. Do not invent new entries.
- [ ] **Step 4: Gate and commit** with an explicit pathspec.

---

## Verification before reporting Phase C complete

1. `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo clippy --features librarian,dashboard -- -D warnings` clean.
2. Focused suites green: `--lib guide_ledger`, `--lib guide_hint_tests`, `--lib prompts`, `--lib rendezvous`, `--lib session_key`. **Not** the full suite — a concurrent session is mid-edit in this checkout.
3. **The measurement that justifies the phase.** The spec's §Problem quantified the waste from `usage.db`. Re-run that measurement, or state plainly that it was not re-run and why. Shipping the fix for a measured problem without re-measuring is how a phase ends up believed rather than known.
4. Every commit verified with `git show --stat` to contain only its own files.

## Deliberately NOT in this plan

- **The `post_compact` clear** (`src/tools/config/mod.rs:278`). Compaction genuinely summarises guide bodies out of context; a full wipe is correct there. Ruling 3.
- **Persisting a project root** — Ruling 2, decided with the user.
- **Threading `Rendezvous` onto `ToolContext`** — Ruling 1; 126 construction sites for no benefit over the ledger flag.
- **The `_meta` rank-3 session key** — deferred in Phase B and still deferred; no client sends it.
- **The four stale bug-file path citations** flagged by `audit_doc_refs` at `med`, below CI's `--fail-on high`. Triaged as accepted by Phase B's final review.
