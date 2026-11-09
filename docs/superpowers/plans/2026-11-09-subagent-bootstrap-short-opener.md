# Subagent Bootstrap Short Opener Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A subagent's session opener ships a short `project-activation-bootstrap` body (≤1,024 B) instead of the whole guide (3,107 B); the parent and every explicit `get_guide` keep the full body.

**Architecture:** `dispatch_tool_call` sets a per-call `caller_is_subagent` flag on the resolved `GuideLedger`, beside `set_session_start_source`, from `asserted_agent.is_some()`. `engines::emitters::emit_session_opener` reads the flag and builds a short block in `guide_emit.rs` with its own marker and a new `GuideDeliveryShape::Short`, whose `_guide_hint` points at the full guide. The ledger stamps the same topic key on both paths. The census learns the new marker form.

**Tech Stack:** Rust (codescout server), Python 3 (census script, unittest).

**Spec:** `docs/superpowers/specs/2026-11-09-subagent-bootstrap-short-variant-design.md` (commit `d16b08ec9`). Recon gaps in the spec: `docs/trackers/context-injection-session-log.md` F-20.

## Global Constraints

- No new entry in `GUIDE_TOPICS`; the short body is a second body for `project-activation-bootstrap`, not a topic.
- Do not decide "subagent" from the ledger key's shape; use `asserted_agent.is_some()` in `dispatch_tool_call`.
- An unstamped call (parent, or any client without the companion) must get exactly today's bytes.
- The short body carries the literal `ALWAYS VERIFY` (pinned by `session_opens_with_bootstrap_from_a_non_workspace_tool` for the full body; F-20).
- Every marker literal lives in `src/tools/core/guide_emit.rs` (`test_markers_match_the_emitter` reads that file).
- Shared checkout: commit by pathspec only; `git add` a new file before the pathspec commit; never touch other sessions' staged paths.

## Review Focus

1. A subagent resumed after a `subagent_rearm` re-arm (ledger non-empty, bootstrap key removed): it must get the short form again, not the whole one. Pinned in Task 1 by a test that seeds the ledger with another topic first.
2. The flag must not leak between principals sharing one server: a subagent call followed by a parent call on the same server must give the parent the whole body. Pinned in Task 1.
3. `_guide_hint` for the short form must never say "do not re-call get_guide" (F-20). Pinned in Task 1 (`inject_hint` test).
4. A stamp with no `/` (`asserted_agent == None`) is the parent's case and must get the whole body. Covered by the parent test (principal `None`); the existing `principal_from_arguments` tests own the parsing.
5. Other transcript parsers (`scripts/measure/join.py`, `scripts/probe_guide_injection.py`) count openings by the `auto-injected get_guide('<topic>')` prefix, which the new marker keeps. No task changes them; the reviewer checks that the prefix is intact.

---

### Task 1: Short opener in the server

**Files:**
- Create: `src/prompts/guides/project-activation-bootstrap-subagent.md`
- Modify: `src/prompts/mod.rs` (beside `SESSION_OPENING_GUIDE`), `src/tools/guide_ledger.rs` (`GuideLedger`), `src/tools/core/guide_emit.rs` (`GuideDeliveryShape`, `inject_hint`, new block builder), `src/engines/emitters.rs` (`emit_session_opener`), `src/server.rs` (`dispatch_tool_call`, beside `set_session_start_source`)
- Test: `src/server.rs` `guide_hint_tests`, `src/tools/core/guide_emit.rs` tests, `src/prompts/mod.rs` tests

**Interfaces:**
- Produces: `pub const SUBAGENT_OPENING_BODY: &str` in `crate::prompts` (`include_str!("guides/project-activation-bootstrap-subagent.md")`).
- Produces: `GuideLedger::set_caller_is_subagent(&mut self, is: bool)` and `GuideLedger::caller_is_subagent(&self) -> bool`; field `caller_is_subagent: bool`, not persisted, default `false`.
- Produces: `GuideDeliveryShape::Short`.
- Produces: `pub(crate) fn subagent_opener_block(topic: &str) -> Content` in `guide_emit.rs`. Open marker `<!-- auto-injected get_guide('{topic}') subagent — first call this session that triggers the topic, short form. get_guide('{topic}') returns the full guide. -->`, close marker `<!-- end auto-injected get_guide('{topic}') subagent -->`, built through `trailing_block`. Task 2 consumes both marker strings.

- [ ] **Step 1: Write the short body file** with exactly this content:

```markdown
# Project Activation Bootstrap (subagent)

Your parent's brief comes first. Load only what your task needs:

- **Memory:** `memory(action="read", topic=...)`; `gotchas`, `conventions`, `architecture` usually pay off; `memory(action="list")` if unsure.
- **Bug or regression work:** `doc(action="find", kind="bug", filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})`. `status="open"` alone hides `taken` (held by a live session).
- **Contract guides:** `get_guide(topic)` when the task touches one: `error-handling`, `progressive-disclosure`, `workspace-state`, `librarian`, `tracker-conventions`, `untrusted-content`.
- **Lookups:** name → `symbols(name=X)`; concept → `semantic_search(query)`; exact string → `grep(pattern)`; callers → `references(symbol, path)`, never grep.

**Do not hypothesise but ALWAYS VERIFY:** a finding needs lines you read; a claim about a tool needs the call run once.

Full orientation: `get_guide("project-activation-bootstrap")`.
```

- [ ] **Step 2: Write the failing tests**

In `src/prompts/mod.rs` tests:

```rust
#[test]
fn subagent_opening_body_is_short_and_keeps_the_used_steps() {
    let b = SUBAGENT_OPENING_BODY;
    assert!(b.len() <= 1024, "short body is {} B", b.len());
    assert!(b.contains("ALWAYS VERIFY"));
    assert!(b.contains(r#"filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}}"#));
    assert!(b.contains(r#"get_guide("project-activation-bootstrap")"#));
}
```

In `guide_emit.rs` tests, extend `inject_hint_tells_the_truth_about_what_shipped` (or add a sibling) with: `inject_hint(&mut v, "t", GuideDeliveryShape::Short)` → the hint contains `get_guide(\"t\")` and does not contain `do not re-call`.

In `server.rs` `guide_hint_tests`, using `make_server` and `dispatch_blocks(server, "run_command", json!({"command": "echo hi"}), principal)`; let `OPEN_SHORT = "<!-- auto-injected get_guide('project-activation-bootstrap') subagent"`, `OPEN_WHOLE = "<!-- auto-injected get_guide('project-activation-bootstrap') — first call"`, `FULL_ONLY = "## Phase 2"`:

- `a_subagent_opener_ships_the_short_body` — principal `Some("conv-A/testagent")`: joined text contains `OPEN_SHORT` and `ALWAYS VERIFY`, does not contain `OPEN_WHOLE` or `FULL_ONLY`.
- `a_parent_opener_still_ships_the_whole_guide` — principal `None`: contains `OPEN_WHOLE` and `FULL_ONLY`, not `OPEN_SHORT`.
- `the_subagent_flag_does_not_leak_to_the_parent` — on ONE server, first the subagent call, then a parent call (`None`): the parent's text contains `OPEN_WHOLE`.
- `an_explicit_fetch_after_the_short_opener_returns_the_full_guide` — subagent call, then `dispatch_text(server, "get_guide", json!({"topic": "project-activation-bootstrap"}), Some("conv-A/testagent"))` contains `FULL_ONLY`.
- `a_re_armed_subagent_gets_the_short_form_again` — subagent call; re-arm that principal's ledger for the bootstrap key the way the existing `subagent_rearm` tests do (a resume request file, or `re_arm(&[SESSION_OPENING_GUIDE], RearmCause::SubagentRearm)` on its ledger); the next subagent call contains `OPEN_SHORT`, not `OPEN_WHOLE`.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `scripts/with-slot.sh cargo test --no-default-features --lib -- subagent_opening_body a_subagent_opener a_parent_opener_still the_subagent_flag an_explicit_fetch_after_the_short a_re_armed_subagent_gets_the_short inject_hint`
Expected: compile errors for `SUBAGENT_OPENING_BODY` and `GuideDeliveryShape::Short`; once those names exist as stubs, the subagent tests FAIL on `OPEN_SHORT` and the parent test PASSES.

- [ ] **Step 4: Implement the Interfaces above**

`emit_session_opener`: when `ledger.caller_is_subagent()`, build `subagent_opener_block(topic)` and return `GuideDeliveryShape::Short`; otherwise keep today's `guide_block` + `Whole` path unchanged. Stamp `topic` in the ledger on both paths. `inject_hint` `Short` arm: `"First call this session for topic '{topic}'. Its short subagent form is below; call `get_guide(\"{topic}\")` for the full guide."`. In `dispatch_tool_call`, call `ledger.lock().set_caller_is_subagent(asserted_agent.is_some())` beside `set_session_start_source`, before `poll_guide_rearm` and before `asserted_agent` is moved.

- [ ] **Step 5: Run the tests to verify they pass, then the whole guide-hint suite**

Run: the Step 3 command, then `scripts/with-slot.sh cargo test --no-default-features --lib guide_hint_tests emitters guide_emit guide_ledger prompts::tests`
Expected: all PASS. Any existing test that expected the WHOLE body for a stamped subagent is a finding: read it, and either it asserts opener *firing* (keep it, it passes on the shared prefix) or it asserts whole content for a subagent (update it, and ledger the ruling).

- [ ] **Step 6: Commit**

```bash
git add src/prompts/guides/project-activation-bootstrap-subagent.md
git commit -m "feat(guides): a subagent's session opener ships a short bootstrap body (bug 7579b32b1cd2362f, F-20)" -- src/prompts/guides/project-activation-bootstrap-subagent.md src/prompts/mod.rs src/tools/guide_ledger.rs src/tools/core/guide_emit.rs src/engines/emitters.rs src/server.rs
```

### Task 2: The census tells the short form apart

**Files:**
- Modify: `scripts/census_guide_deliveries.py` (`OPEN`, `_deliveries_in`)
- Test: `tests/test_census_guide_deliveries.py`

**Interfaces:**
- Consumes: the two marker strings from Task 1's `subagent_opener_block`.
- Produces: delivery rows with `form == "subagent"`.

- [ ] **Step 1: Write the failing tests**

Add a `subagent(topic, body="short")` helper built from Task 1's open and close markers. Add `test_subagent_form_is_told_apart`: a result item `subagent("project-activation-bootstrap") + "\n" + whole("symbol-navigation")` yields forms `{("project-activation-bootstrap", "subagent"), ("symbol-navigation", "whole")}`, and the subagent row has `closed == True`. Add both marker strings (with `{topic}`) to `test_markers_match_the_emitter`'s tuple.

- [ ] **Step 2: Run to verify it fails**

Run: `python3 -m unittest tests.test_census_guide_deliveries -v`
Expected: `test_subagent_form_is_told_apart` FAILS (no row, or form `whole` with `closed == False`); the marker test PASSES only if Task 1 is committed.

- [ ] **Step 3: Implement**

Add `| subagent — first call` to `OPEN`'s alternation and a `"subagent"` branch in `_deliveries_in` whose close is `<!-- end auto-injected get_guide('{topic}') subagent -->`, checked before the `whole` fallback.

- [ ] **Step 4: Run to verify it passes**

Run: `python3 -m unittest tests.test_census_guide_deliveries -v`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(census): count the subagent short opener as its own form (bug 7579b32b1cd2362f)" -- scripts/census_guide_deliveries.py tests/test_census_guide_deliveries.py
```

### Task 3: Gate and record

**Files:**
- Modify: `docs/issues/2026-08-27-guide-topics-are-atomic-nodes-in-an-unmodelled-graph.md` (new section via `doc(update, body_edits)`)
- Modify: `docs/trackers/context-injection-session-log.md` F-20 status (via `doc(update)`)

- [ ] **Step 1: Run the gate**

Run: `./scripts/gate.sh`
Expected: exits 0. A red caused by another session's work is reported by name, not fixed here.

- [ ] **Step 2: Measure the shipped bytes**

Run one stamped and one unstamped opener in a test or a dev binary and record both block sizes in bytes.
Expected: the short block is under 1,200 B wrapped; the whole block is unchanged from before Task 1.

- [ ] **Step 3: Record**

Append a section "Shipped 2026-11-09: subagent short opener" to the issue with the two byte counts, the commits, and the still-open items (withholding experiment, section split). Set F-20 to `fixed-verified` with the commit SHA. Commit both by pathspec.
