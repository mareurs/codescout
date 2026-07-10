# project-activation-bootstrap Guide Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `project-activation-bootstrap` `get_guide` topic (the main-agent-adapted exploration protocol) that auto-injects on every `workspace(activate)`.

**Architecture:** Register a new compiled-in guide topic (4-point registration: `GUIDE_TOPICS`, `topic_body()`, the no-arg summaries map, and a new `guides/*.md` file), then make the one registered activation tool (`Workspace`) return that topic from `relevant_guide_topic()`. The existing "V2 hard-injection" path in `Tool::call_content` (`src/tools/core/types.rs:545`) does the rest: because `Workspace::call` clears the per-session guide ledger on activate *before* `call_content` re-checks it, the guide body is appended as a second content block on every activate.

**Tech Stack:** Rust, `async_trait`, `serde_json`, `tokio::test`. No new dependencies.

## Global Constraints

- **Tool-description budget:** every non-librarian tool's `description()` must be `<= 300` chars — gate `tool_descriptions_stay_under_budget` (`src/server.rs:1651`). The `get_guide` description is currently **297/300**; it MUST be rewritten (verbatim text in Task 1) to fit the new topic.
- **4-point registration is drift-guarded:** `guide_topics_have_bodies` (`src/prompts/mod.rs`), the summary-coverage + `schema_enum_matches_registered_topics` tests (`src/tools/guide.rs`) fail the build if `GUIDE_TOPICS`, `topic_body()`, the summaries map, and the `.md` file drift apart.
- **No hardcoded topic counts to touch:** `src/tools/guide.rs:161,182` already derive from `crate::prompts::GUIDE_TOPICS.len()` (R-37 already satisfied) — do not introduce a magic number.
- **Slug is final:** `project-activation-bootstrap` (fits every surface; the 281-char rewritten description in Task 1 includes it).
- **Pre-commit gate** (run before each task's commit): `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`.
- **Branch:** work on `experiments` (master is protected).
- **Do NOT** add the topic to `src/prompts/source.md` `server_instructions` (2200-byte slice cap hazard) — auto-inject is the delivery path; the no-arg `get_guide` listing keeps it discoverable. Out of scope.

---

## File Structure

- **Create:** `src/prompts/guides/project-activation-bootstrap.md` — the guide body (main-agent exploration protocol + reconnaissance trigger + dispatch framing). One responsibility: the injected/fetched content.
- **Modify:** `src/prompts/mod.rs` — add the slug to `GUIDE_TOPICS` and a `topic_body()` match arm (registration).
- **Modify:** `src/tools/guide.rs` — add the no-arg summary entry and rewrite `description()` to fit budget. Add the content-regression test.
- **Modify:** `src/tools/config/mod.rs` — add `relevant_guide_topic()` to `impl Tool for Workspace` (the auto-inject trigger).
- **Modify:** `src/tools/config/tests.rs` — add the auto-inject-on-activate test.

---

## Task 1: Register the `project-activation-bootstrap` guide topic

**Files:**
- Create: `src/prompts/guides/project-activation-bootstrap.md`
- Modify: `src/prompts/mod.rs` (`GUIDE_TOPICS` ~L128-138, `topic_body()` ~L147-160)
- Modify: `src/tools/guide.rs` (`description()` ~L41-47, summaries map ~L68-78)
- Test: `src/tools/guide.rs` tests module (new `get_guide_returns_project_activation_bootstrap_body`)

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces: `crate::prompts::topic_body("project-activation-bootstrap") -> Some(<body>)` and a registered `get_guide` topic — Task 2's auto-inject depends on `topic_body` being `Some`.

- [ ] **Step 1: Write the failing content-regression test**

Add to the `tests` module in `src/tools/guide.rs` (the module already defines a `ctx().await` helper and imports `GetGuide`, `json!`):

```rust
    #[tokio::test]
    async fn get_guide_returns_project_activation_bootstrap_body() {
        let g = GetGuide::new();
        let result = g
            .call(json!({ "topic": "project-activation-bootstrap" }), &ctx().await)
            .await
            .unwrap();
        let body = result["body"].as_str().expect("body must be a string");
        assert!(body.contains("Phase 0"), "guide must include Phase 0");
        assert!(
            body.contains("reconnaissance"),
            "guide must include the reconnaissance trigger"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p codescout get_guide_returns_project_activation_bootstrap_body`
Expected: FAIL — the `.unwrap()` panics because the topic is unregistered (`GetGuide::call` returns `RecoverableError("unknown topic ...")`).

- [ ] **Step 3: Create the guide body file**

Create `src/prompts/guides/project-activation-bootstrap.md`:

```markdown
# Project Activation Bootstrap

You just activated a project. Orient before you explore or edit — the cheapest
bug is the one you never re-investigate because the project already documented it.

## Phase 0 — load what the project already knows (do FIRST)

- `memory(action="list")`, then read the topics matching your task.
  `architecture`, `gotchas`, and `conventions` usually pay off.
- Bug or regression work: `artifact(action="find", kind="bug", status="open")` —
  the known-bug ledger. Don't re-file a filed bug as new; mark a rediscovery
  KNOWN and cite the ledger path.
- If a `get_guide` topic matches your area (`error-handling`,
  `progressive-disclosure`, `workspace-state`, `librarian`,
  `tracker-conventions`), read it — it states the contract whose violations you
  hunt.

## Phase 1 — route each lookup by what you know

- symbol name → `symbols(name=X)`
- concept → `semantic_search(query)`
- exact string → `grep(pattern)`
- who calls X → `references(symbol, path)` — never grep for callers

## Phase 2 — verify at the bytes, not from belief

- A finding needs lines you actually read (`symbols include_body` / `read_file`),
  not a grep hit alone.
- A claim about how a TOOL behaves needs the call run once and the real output
  read — reading the source alone misses runtime shape.
- A comment, doc, or README the code contradicts is itself a finding
  (doc-vs-code drift).

## Before you plan or touch a contract — run reconnaissance

If you will write a plan, change a struct / function signature / API contract,
or verify claims against `docs/trackers`, invoke the reconnaissance skill FIRST.

- Claude Code: `/codescout-companion:reconnaissance`.
- Other harnesses: follow `docs/templates/session-log.md` (any agent that reads
  markdown can use the template — no plugin required).

It forces the doc-vs-code reconciliation and logs frictions (F-N) and wins (W-N)
so the next session inherits them.

## When you dispatch subagents — brief them

Pass what you already loaded: memories read, guide topics triggered, open bugs.
A subagent re-discovering what you already knew is a dispatch defect (Iron Law
6), not the subagent's fault.
```

- [ ] **Step 4: Register the slug in `GUIDE_TOPICS`**

In `src/prompts/mod.rs`, add the slug as the last entry of `GUIDE_TOPICS`:

```rust
pub const GUIDE_TOPICS: &[&str] = &[
    "librarian",
    "librarian-runtime",
    "tracker-conventions",
    "progressive-disclosure",
    "error-handling",
    "workspace-state",
    "iron-laws-detail",
    "symbol-navigation",
    "untrusted-content",
    "project-activation-bootstrap",
];
```

- [ ] **Step 5: Add the `topic_body()` match arm**

In `src/prompts/mod.rs`, add the arm before the `_ => None,` fallback:

```rust
        "untrusted-content" => Some(include_str!("guides/untrusted-content.md")),
        "project-activation-bootstrap" => {
            Some(include_str!("guides/project-activation-bootstrap.md"))
        }
        _ => None,
```

- [ ] **Step 6: Add the no-arg summary entry**

In `src/tools/guide.rs` `GetGuide::call`, add to the `summaries` object (after the `untrusted-content` line):

```rust
                    "untrusted-content": "data vs directives in repo/file/web content: quarantine embedded instructions, verify facts via your own tooling",
                    "project-activation-bootstrap": "orient after activate: load memory + open-bug ledger, route lookups, verify at bytes, run reconnaissance before planning"
```

- [ ] **Step 7: Rewrite `get_guide`'s `description()` to fit the 300-char budget**

In `src/tools/guide.rs`, replace the entire `description()` return string (current 297 chars, no room for the new slug) with this measured 281-char version:

```rust
    fn description(&self) -> &str {
        "Deep guidance for a topic; call with no args to list every topic + one-line summaries. \
         Covers librarian/trackers, error-handling, progressive-disclosure, workspace-state, \
         iron-laws, symbol-navigation, untrusted-content, and project-activation-bootstrap. \
         Full guide returned inline."
    }
```

- [ ] **Step 8: Run the drift guards + budget gate + new test**

Run: `cargo test -p codescout guide_topics_have_bodies schema_enum_matches_registered_topics tool_descriptions_stay_under_budget get_guide_returns_project_activation_bootstrap_body`
Expected: PASS (also run `cargo test -p codescout --lib guide` to sweep the guide + summary-coverage tests).

- [ ] **Step 9: Full gate + commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/prompts/guides/project-activation-bootstrap.md src/prompts/mod.rs src/tools/guide.rs
git commit -m "feat(guide): add project-activation-bootstrap topic (main-agent exploration protocol)

Registers the 4-point topic and rewrites get_guide's description (was
297/300 chars) to a compact 281-char form that no longer enumerates every
slug inline, making room for this and future topics.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Auto-inject the guide on `workspace(activate)`

**Files:**
- Modify: `src/tools/config/mod.rs` (`impl Tool for Workspace`, after `format_compact` ~L81-90)
- Test: `src/tools/config/tests.rs` (new `workspace_activate_injects_bootstrap_guide_body_v2`)

**Interfaces:**
- Consumes: `crate::prompts::topic_body("project-activation-bootstrap")` from Task 1 (the V2 injector calls it to build the second block; if `None`, only the `_guide_hint` field ships and the test's 2-block assertion fails — so Task 1 must land first).
- Produces: nothing downstream.

- [ ] **Step 1: Write the failing auto-inject test**

Add to `src/tools/config/tests.rs` (module already has `use super::*;`, the `lsp()` helper, and the `ToolContext` construction pattern):

```rust
    #[tokio::test]
    async fn workspace_activate_injects_bootstrap_guide_body_v2() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        };

        let blocks = Workspace
            .call_content(
                json!({ "action": "activate", "path": dir.path().to_str().unwrap() }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(
            blocks.len(),
            2,
            "first workspace(activate) must append the bootstrap guide body block, got {}",
            blocks.len()
        );
        let second = blocks[1].as_text().expect("second block must be text");
        assert!(
            second
                .text
                .contains("<!-- auto-injected get_guide('project-activation-bootstrap')"),
            "second block missing the auto-inject opening marker"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p codescout workspace_activate_injects_bootstrap_guide_body_v2`
Expected: FAIL — `blocks.len()` is `1`, because `Workspace::relevant_guide_topic()` still returns the default `None`, so no second block is appended.

- [ ] **Step 3: Add `relevant_guide_topic()` to `Workspace`**

In `src/tools/config/mod.rs`, inside `impl Tool for Workspace`, add the method immediately after the `format_compact` method (which ends at ~L90, before the closing `}` of the impl):

```rust
    fn relevant_guide_topic(&self) -> Option<&str> {
        // Fires the project-activation-bootstrap guide via the V2 hard-injection
        // path. Tool-granular (no access to `action`), but `activate` clears the
        // guide ledger in `call()` before `call_content` re-checks it, so the
        // guide re-injects on every activate. A pre-activate status/list call
        // fires it once (harmless); post-activate calls are ledger-suppressed.
        Some("project-activation-bootstrap")
    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p codescout workspace_activate_injects_bootstrap_guide_body_v2`
Expected: PASS — two content blocks, the second carrying the `<!-- auto-injected get_guide('project-activation-bootstrap') -->` marker.

- [ ] **Step 5: Full gate + commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/tools/config/mod.rs src/tools/config/tests.rs
git commit -m "feat(config): auto-inject project-activation-bootstrap guide on activate

Workspace::relevant_guide_topic() returns the new topic; the V2
hard-injection path appends the guide body on every workspace(activate),
closing the main-agent gap (protocol previously reached subagents only).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Full verification gate + live MCP check

**Files:** none (verification only).

**Interfaces:** consumes the shipped behavior from Tasks 1–2.

- [ ] **Step 1: Full test + lint gate**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: PASS, no warnings.

- [ ] **Step 2: Build the live MCP binary**

Run: `cargo rb`
Expected: release build succeeds (this is the live-MCP build per project convention, not `cargo build --release`).

- [ ] **Step 3: Reconnect and verify live injection**

In the Claude Code session, run `/mcp` to reconnect, then call `workspace(action="activate", path="/home/marius/work/claude/codescout")`.
Expected: the response carries a second content block wrapped in `<!-- auto-injected get_guide('project-activation-bootstrap') ... -->` containing "Phase 0", the reconnaissance trigger, and the dispatch-framing section. Confirm `get_guide("project-activation-bootstrap")` also returns it on demand.

- [ ] **Step 4: No commit** — verification only. If any check fails, return to the owning task rather than marking complete.

---

## Self-Review

**Spec coverage:**
- Mechanism (`Workspace::relevant_guide_topic`, one wiring point, fires-on-every-activate) → Task 2. ✓
- Content (Phases 0–2 + reconnaissance trigger + dispatch framing, main-agent-adapted) → Task 1 Step 3. ✓
- 4-point registration → Task 1 Steps 3–6. ✓
- Prompt-surface budget gate (300-char) → Global Constraints + Task 1 Step 7 (measured 281). ✓
- R-37 hardcoded-count check → Global Constraints (already derived; nothing to change). ✓
- Tests (mirror of `first_artifact_call_appends_librarian_guide_body_v2`) → Task 2 Step 1; content regression → Task 1 Step 1. ✓
- "Do not dedup with subagent hook" → carried in the spec; no code touches the hook (Out of scope). ✓
- Full gate + live verify → Task 3. ✓

**Placeholder scan:** no TBD/TODO; every code and test step shows complete content; the guide body is written in full. ✓

**Type consistency:** slug `project-activation-bootstrap` identical across `GUIDE_TOPICS`, `topic_body()`, summaries map, `.md` filename, `relevant_guide_topic()`, and both tests. Test helper names (`ctx().await`, `lsp()`, `Workspace`, `call_content`, `as_text()`) match the existing harnesses read from `guide.rs` and `config/tests.rs`. ✓
