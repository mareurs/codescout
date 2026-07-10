# Design — `project-activation-bootstrap` guide, auto-injected on activate

**Date:** 2026-07-10
**Status:** approved (brainstorming)
**Author:** Marius + Claude (Opus 4.8)

## Problem

The three-phase **exploration protocol** — load project knowledge first, route
each lookup by what you know, verify at the bytes — currently lives in exactly
one place: the `SubagentStart` hook `subagent-guidance.sh` in the
`claude-plugins` repo (commit `841ee93`, "exploration protocol v1.1"). That hook
reaches **subagents only**.

The **main agent never sees the protocol.** It receives codescout's
`server_instructions` (the terse CODESCOUT RULES / Iron Laws block) but not the
Phase 0/1/2 discipline. This is a structural asymmetry: subagents can't receive
`server_instructions` (claude-code#29655), so the protocol was delivered to them
out-of-band via the hook; the main agent's in-process guide channel was never
given an equivalent.

Provenance for the protocol's value: codescout
`docs/trackers/prompt-hamsa-audit-log.md` A-16 (3-arm bug-hunt A/B, sonnet,
n=3/arm) and A-15 (0/10 memory-blind subagents — subagents that never loaded
project memory). Phase 0 is the direct fix for that 0/10 failure mode.

## Goal

Give the **main agent** the same orientation discipline at the moment it makes
sense — right after it activates a project — delivered through codescout's
existing in-process guide auto-injection, as a new `get_guide` topic
`project-activation-bootstrap` that fires automatically on `workspace(activate)`.

Non-goal: single-sourcing the guide with the subagent hook. The two audiences
differ (see "Relationship to the subagent hook"), so the guide is an *adapted*
translation, not a shared copy. They are allowed to diverge.

## Mechanism

codescout already has "V2 hard-injection" (`src/tools/core/types.rs:531`+): any
tool whose `relevant_guide_topic()` returns `Some(topic)` gets the **full guide
body appended as a second `Content::text` block**, wrapped in
`<!-- auto-injected get_guide('TOPIC') ... -->` markers, on the first call of the
session that triggers the topic. A per-session ledger (`guide_hints_emitted`,
`src/tools/guide_ledger.rs`) gates it; the ledger is **cleared on workspace
activate** and on post-compact re-arm (`guide_ledger.rs:59`).

**Fire timing (verified against `types.rs:545`).** In the default `call_content`:

```
let val = self.call(input, ctx).await?;         // 1. runs FIRST
...
if let Some(topic) = self.relevant_guide_topic() // 2. THEN checks ledger
    let mut emitted = ctx.guide_hints_emitted.lock();
    if emitted.contains(topic) { None } else { insert + fire }
```

For the activate path, step 1 (`Workspace::call` → `ActivateProject::call`)
**clears the ledger** before step 2 reads it. So the ledger is empty when
checked ⇒ the topic fires on **every** activate (first activation and every
project switch / return-to-home). This is the same re-arm behaviour
`progressive-disclosure` already has, and it is the behaviour we want.

### Wiring point — exactly one

Only `Workspace` is registered as a tool (`src/server.rs:131`,
`Arc::new(Workspace)`). `activate_project` / `ActivateProject` is **not** a
standalone registered tool — it is an internal struct dispatched by
`Workspace::call` via `.call()` (not `.call_content()`), so its
`relevant_guide_topic()` is never consulted. The single injection point is:

```rust
// src/tools/config/mod.rs — impl Tool for Workspace
fn relevant_guide_topic(&self) -> Option<&str> {
    Some("project-activation-bootstrap")
}
```

### Known tradeoff — tool-granular, not action-granular

`relevant_guide_topic(&self)` receives no input, so it cannot branch on
`action`. Firing is therefore per-tool:

- `workspace(action="activate")` — `call()` clears the ledger ⇒ **fires.** ✓
- `workspace(action="status" | "list_projects")` issued **before any activate** —
  ledger not cleared, topic not yet emitted ⇒ fires **once** (harmless; activate
  is effectively always the session's first action).
- `status` / `list_projects` **after** an activate has fired it — topic already
  emitted, ledger not re-cleared ⇒ **suppressed.** ✓

**Decision:** accept this. The one imprecise case (a `status` as the very first
workspace call of a session) is rare and low-cost. The rejected alternative —
override `Workspace::call_content` to inject only when `action=="activate"` and
optionally suppress on return-to-home — buys action precision at the cost of
duplicating the marker-wrapping + ledger logic the default `call_content`
already performs. Not worth it for v1. Re-injection on return-to-home is treated
as acceptable (re-brief on context switch), matching `progressive-disclosure`.

## Content — `src/prompts/guides/project-activation-bootstrap.md`

An adaptation of the subagent protocol for the **orchestrating main agent**. It
keeps Phases 0–2 and replaces the subagent report contract with dispatch
framing, and adds the reconnaissance trigger.

Sections:

1. **Intro.** One line: you've just activated a project; orient before you
   explore or edit.
2. **Phase 0 — load what the project already knows (do first).**
   - `memory(action="list")`, then read the topics matching your task
     (architecture, gotchas, conventions usually pay off).
   - Bug / regression work: `artifact(action="find", kind="bug",
     status="open")` — the known-bug ledger. Don't re-file a known bug as new;
     mark rediscoveries KNOWN with the ledger path.
   - If a `get_guide` topic matches your area (error-handling,
     progressive-disclosure, workspace-state, librarian, tracker-conventions),
     read it — it states the contract whose violations you hunt.
3. **Phase 1 — route each lookup by what you know.** symbol name →
   `symbols(name=X)` | concept → `semantic_search(query)` | exact string →
   `grep(pattern)` | who calls X → `references(symbol, path)`, never grep for
   callers.
4. **Phase 2 — verify at the bytes, not from belief.** A finding needs lines you
   actually read (`symbols include_body` / `read_file`), not a grep hit alone.
   A claim about how a **tool** behaves needs the call run once and the real
   output read. A comment / doc / README the code contradicts is itself a
   finding (doc-vs-code drift).
5. **Reconnaissance trigger (new).** If you will **write a plan**, change a
   **struct / function signature / API contract**, or **verify claims against
   `docs/trackers`**, invoke the reconnaissance skill *first*. Claude Code:
   `/codescout-companion:reconnaissance`. Other harnesses: follow
   `docs/templates/session-log.md` (any agent that can read markdown can use the
   template — no plugin required). It enforces the doc-vs-code reconciliation and
   logs frictions (F-N) and wins (W-N).
6. **Dispatch framing (replaces the subagent "Ledger checked:" report
   contract).** When you dispatch subagents, brief them with what you already
   loaded — memories read, guide topics triggered, open bugs. A subagent
   re-discovering what you already knew is a dispatch defect (Iron Law 6), not
   the subagent's fault.

Style: match the terse, imperative voice of the existing `guides/*.md`. Keep it
short — it injects on every activate, so every line pays rent.

## Registration (4-point, drift-guarded)

Adding a topic touches four coupled surfaces; the build fails if any drift
(tests `guide_topics_have_bodies` and `schema_enum_matches_registered_topics` in
`src/prompts/mod.rs` / `src/tools/guide.rs`):

1. `GUIDE_TOPICS` in `src/prompts/mod.rs` — add the slug.
2. `topic_body()` in `src/prompts/mod.rs` — add
   `"project-activation-bootstrap" => Some(include_str!("guides/project-activation-bootstrap.md"))`.
3. The no-arg `summaries` map in `src/tools/guide.rs` `GetGuide::call` — add a
   one-line summary.
4. Create `src/prompts/guides/project-activation-bootstrap.md`.

## Prompt-surface budget gates (reconnaissance R-28 / R-37)

The `reconnaissance` memory rule fires: before editing enumerated prompt
surfaces, enumerate and run their budget/count gates *first*.

- **`get_guide` description budget.** `GetGuide::description` (`guide.rs:41`)
  enumerates every topic slug inline, and the no-arg `summaries` map grows too.
  Check the tool-description length gate in `server::tests` (the ~300-char
  budget). `project-activation-bootstrap` is 28 chars; if it busts the budget,
  fall back to the shorter slug **`activation-bootstrap`** (20 chars). Slug
  choice is finalized during implementation against the measured budget.
- **Hardcoded topic counts.** grep the guide-surface tests for `len() == N` /
  literal topic-count assertions; convert any to derive from `GUIDE_TOPICS.len()`
  while here (R-37), rather than bumping a magic number.

## Tests

- **New:** assert `Workspace::relevant_guide_topic()` returns the slug, and that
  a first `workspace(action="activate")` `call_content` response contains the
  `<!-- auto-injected get_guide('project-activation-bootstrap') -->` block —
  mirroring `first_artifact_call_appends_librarian_guide_body_v2`
  (`src/server.rs:3226`).
- **Existing drift guards** (`guide_topics_have_bodies`,
  `schema_enum_matches_registered_topics`) automatically cover points 1–4 of
  registration once the slug is added.
- Full gate before done: `cargo fmt`, `cargo clippy -- -D warnings`,
  `cargo test`; then `cargo rb` + `/mcp` to verify the guide injects live on the
  next activate.

## Relationship to the subagent hook (do NOT dedup)

`subagent-guidance.sh` (claude-plugins) and this guide (codescout) intentionally
carry parallel-but-distinct copies:

- **Different repos** — no cross-repo single-source is practical.
- **Different audiences** — the hook version keeps the subagent report contract
  ("cite file:line; end with `Ledger checked:`"); this guide replaces it with
  dispatch framing for the orchestrating agent.

A future maintainer tempted to "deduplicate these" should read this section
first: they serve different callers by design. If the shared Phases 0–2 wording
drifts materially, reconcile *those phases*, not the audience-specific tails.

## Out of scope / YAGNI

- Action-aware injection (`call_content` override) — deferred; the one-liner is
  sufficient.
- Return-to-home suppression — deferred; re-brief on switch is acceptable.
- Changing the subagent hook — untouched by this work.
