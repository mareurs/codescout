---
title: Goal-Tracker Archetype — Design
status: proposed
created: 2026-05-16
owner: marius
audience: claude (during work sessions) + future contributors editing tracker archetypes
related:
  - crates/librarian-mcp/src/tools/tracker_design.rs
  - docs/superpowers/specs/2026-05-01-artifact-augmentation-design.md
  - docs/superpowers/specs/2026-05-09-bug-tracker-template-design.md
  - /home/marius/work/claude/playground/research/claude-code-2026-05-features.md
brainstorm_specialists:
  - architecture-snow-lion
  - prompt-hamsa
---

# Goal-Tracker Archetype — Design

## Goal

Introduce a `goal` tracker archetype that represents an outcome-stated objective with a named completion criterion, decomposes into typed child trackers, and integrates with Claude Code's per-turn `Stop` event so a session-scoped autonomous loop is driven by durable, MCP-resident state rather than ephemeral CC `/goal` syntax.

The archetype must:

- Slot into the existing `librarian-mcp` augmentation pipeline with zero new tool surfaces.
- Aggregate child state mechanically; never compute evidence itself.
- Surface itself to any LLM connecting to codescout via existing discovery affordances (tag + server_instructions paragraph + `librarian_context` extension).
- Replace, not wrap, Claude Code's native `/goal` evaluator via a prompt-based custom `Stop` hook in `codescout-companion`.

## Audience

Claude (the agent) creates and refreshes goal-trackers; the human collaborates by editing prose body sections (rationale, history). The Stop hook is consumed by Claude Code sessions but the goal-tracker itself is agent-agnostic — any MCP client (Copilot, Gemini, Cursor) discovers and reads it via the same tools.

## Out of scope

- **Multi-user / ownership / ACL on goals.** No model for "this is Bob's goal, Alice can't close it." Tracker artifacts are project-scoped, single-tenant for v1.
- **Cross-project goals** spanning multiple workspace projects (e.g., umbrella-scoped goals). Possible via existing workspace umbrella mechanism, deferred.
- **Goal deadlines / SLAs / target dates.** Adding `target_date` / `deadline_action` is speculative; defer until a real use case asks. Decoration risk if added now.
- **Goal templates / forking.** UX add-on with no architectural content; defer.
- **Wrapping native CC `/goal` command.** Web research (see Findings) showed `/goal` exposes no extension surface — no public API surfaces the criterion text, no hook payload field, no status-line variable. We bypass it entirely via a custom `Stop` hook.
- **Status-line overlay integration.** Same finding: no public API on the overlay panel. Out of scope.
- **Optimistic concurrency on `artifact(update)`.** Last-write-wins by `updated_at` is acceptable for v1; revisit if concurrent corruption appears.

## Approach (decision summary)

Three approaches were weighed:

| Option | Shape | Verdict |
|---|---|---|
| A — single `goal` archetype with `goal_type` enum + conditional prompt | One archetype, branching prompt | **Rejected.** Prompt accumulates per-type branches; readability collapses past ~5 types. Hamsa: branching prompt is N prompts pretending to be one — model receives all branches every refresh, must filter to its own type. |
| B — family of sibling archetypes (`goal_tests_pass`, `goal_refactor`, `goal_metric`, `goal_freeform`) | 3–4 archetypes per goal shape | **Rejected.** Premature multiplication. Lion: rule of three guards against shaping abstractions from a sample of one. With only one goal pattern observed, the family is a guess. |
| **C — container archetype with existing archetypes as children** | One archetype, `children[]` of existing kinds | **Adopted.** Type-dependent evidence schema is delegated to the existing archetypes that already model those signal shapes (`failure_table`, `task_list`, `metric_baseline`, `audit_issues`, `reflective`). The new archetype is a thin aggregator. |

Option C honors the user observation that *"what a goal requires depends on the goal type"* by **routing the type-dependence into the child archetype choice**, not into the goal archetype itself. The goal stays minimal and schema-stable; type-specific evidence lives where it already does.

## Findings — Claude Code `/goal` (informing S5)

Web research at 2026-05-16 against `anthropics/claude-code` CHANGELOG (main) and docs.claude.com established four facts that shaped the integration design:

1. **`/goal` is a session-scoped prompt-based `Stop` hook in disguise.** "A built-in shortcut for a session-scoped prompt-based Stop hook." There is no separate evaluator process.
2. **No public API exposes the active goal text** to MCP servers, plugins, status-line scripts, or external hooks. No env var, no hook-payload field, no status-line variable.
3. **Goal text survives `--resume` / `--continue`** but turn/token counters reset; no daemon-side persistence beyond the session record.
4. **Sanctioned integration path is "write your own prompt-based Stop hook"** — Anthropic's docs explicitly position this as the supported escape hatch for custom evaluation logic.

The design adopts the sanctioned path: a custom `Stop` prompt-hook in `codescout-companion` that reads the active goal-tracker artifact and signals stop/continue based on `goal.status`. We give up the native `/goal` overlay panel and the `--resume` carryover for the criterion text; we gain full programmatic control with the goal-tracker as single source of truth.

## Architecture

**One new archetype, zero new code paths** in librarian-mcp beyond:

1. Appending the `goal` entry to `TRACKER_ARCHETYPES` in `crates/librarian-mcp/src/tools/tracker_design.rs`.
2. Adding a no-anchor branch to `librarian(action="context")` that prepends an "Active goals" header when called without `anchor_id` or `topic` (per surfacing layer S3, below).

**Container pattern.** The `goal` archetype is a container: its augmentation prompt instructs the synthesizer to aggregate children's state, not to compute evidence. All evidence lives in children, which are existing archetype kinds linked via the existing `artifact(action="link", rel="child")` edge — no new edge type.

**Boundary responsibilities:**

- *Goal artifact owns:* the criterion (natural-language string), the aggregate status, the children index, the per-refresh progress log, the human-narrative body.
- *Children own:* type-specific evidence (test results, task statuses, metric deltas, audit issues, reflective decisions).
- *Change scenario absorbed:* new goal *types* (refactor, doc, infra rollout, performance regression) require zero new archetypes — they are new *compositions* of existing children.

**Decision record:**

- *Decision:* one container archetype, leverage existing archetypes for evidence.
- *Alternatives rejected:* A — fights the existing archetype catalog; B — premature multiplication of archetypes before three concrete shapes are exhausted.
- *Now easier:* `tracker_design` archetype list stays at 6 + 1; existing archetypes' refresh prompts don't change.
- *Now harder:* the container must *understand* its children's schemas to aggregate. That coupling lives in the augmentation prompt, not in code, so it is tolerable.
- *Confidence:* high.

## Components — the archetype definition

The archetype is registered as the 7th entry in `TRACKER_ARCHETYPES` in `crates/librarian-mcp/src/tools/tracker_design.rs`.

### name + when_to_use

```
name: "goal"
when_to_use: "Tracking an outcome-stated objective whose completion depends on a
named criterion and on aggregated state of sibling/child artifacts. Use when the
work has a definable 'done' line, decomposes into typed sub-trackers (tests,
tasks, metrics, audits), and survives across sessions. Examples: 'all flaky
tests resolved + suite green for 3 runs', 'retrieval P@5 reaches 0.20 on
benchmark X', 'plan-lifecycle subsystem ships behind feature flag'. Not for:
open-ended research (use `reflective`), single-metric tracking (use
`metric_baseline`), bare task lists with no completion semantics (use
`task_list`)."
```

### params_shape_example

```json
{
  "criterion": "Retrieval pipeline P@5 ≥ 0.20 on benchmark-25tc, with no regression on R@5",
  "status": "active",
  "blocked_reason": null,
  "acceptance_signals": [
    {"description": "P@5 ≥ 0.20 on benchmark-25tc", "met": false, "evidence": "metric_baseline child a1b2c3 current=0.193"},
    {"description": "R@5 not below baseline 0.724",   "met": true,  "evidence": "metric_baseline child a1b2c3 current=0.781"},
    {"description": "No new failures in chat-eval-v3", "met": true,  "evidence": "failure_table child d4e5f6 0 fail/12"}
  ],
  "children": [
    {"id": "C-1", "artifact_id": "a1b2c3d4", "title": "Retrieval Benchmark",   "archetype": "metric_baseline", "status": "in-progress"},
    {"id": "C-2", "artifact_id": "d4e5f6a7", "title": "chat-eval-v3 failures",  "archetype": "failure_table",   "status": "active"},
    {"id": "C-3", "artifact_id": "b9c8d7e6", "title": "Reranker tuning tasks",  "archetype": "task_list",        "status": "done"}
  ],
  "progress_log": [
    {"date": "2026-05-12", "note": "Reranker tuning landed. P@5 0.145 → 0.193.", "evidence_commits": ["abc1234"], "evidence_artifacts": ["a1b2c3d4"]},
    {"date": "2026-05-14", "note": "chat-eval-v3 stable. Need final 7pt P@5.",  "evidence_commits": [],          "evidence_artifacts": ["d4e5f6a7"]}
  ]
}
```

### params_schema_example

```json
{
  "type": "object",
  "required": ["criterion", "status", "children"],
  "properties": {
    "criterion":      {"type": "string"},
    "status":         {"type": "string", "enum": ["scoping","active","pending-confirmation","done","blocked","abandoned"]},
    "blocked_reason": {"type": ["string","null"]},
    "acceptance_signals": {"type": "array", "items": {
      "type": "object", "required": ["description","met"],
      "properties": {
        "description": {"type": "string"},
        "met":         {"type": "boolean"},
        "evidence":    {"type": "string"}
      }
    }},
    "children": {"type": "array", "items": {
      "type": "object", "required": ["id","artifact_id","title","archetype","status"],
      "properties": {
        "id":          {"type": "string", "pattern": "^C-\\d+$"},
        "artifact_id": {"type": "string"},
        "title":       {"type": "string"},
        "archetype":   {"type": "string"},
        "status":      {"type": "string", "enum": ["pending","active","in-progress","done","blocked","orphan","unknown"]}
      }
    }},
    "progress_log": {"type": "array", "items": {
      "type": "object", "required": ["date","note"],
      "properties": {
        "date":               {"type": "string"},
        "note":               {"type": "string"},
        "evidence_commits":   {"type": "array", "items": {"type": "string"}},
        "evidence_artifacts": {"type": "array", "items": {"type": "string"}}
      }
    }}
  }
}
```

Schema is locked on `criterion / status / children`, loose on `acceptance_signals` and `progress_log` (additional fields permitted). Matches the teaching prompt's discipline: "loose early, lock when shape settles."

### render_template_example (MiniJinja)

```
**Goal:** {{ criterion }}
**Status:** {{ status }}{% if blocked_reason %} — _blocked: {{ blocked_reason }}_{% endif %}

{% if acceptance_signals %}**Acceptance signals** — {{ acceptance_signals|selectattr("met")|list|length }}/{{ acceptance_signals|length }} met

| signal | met | evidence |
|--------|:---:|----------|
{% for s in acceptance_signals %}| {{ s.description }} | {{ "✅" if s.met else "❌" }} | {{ s.evidence or "—" }} |
{% endfor %}{% endif %}

**Children** — {{ children|selectattr("status","equalto","done")|list|length }}/{{ children|length }} done

| id | title | archetype | status |
|---:|-------|-----------|--------|
{% for c in children %}| {{ c.id }} | {{ c.title }} | {{ c.archetype }} | {{ c.status }} |
{% endfor %}

{% if progress_log %}**Recent progress** _(last 3 of {{ progress_log|length }})_

{% for p in progress_log|reverse|slice(3)|first %}- **{{ p.date }}**: {{ p.note }}
{% endfor %}{% endif %}
```

### body_skeleton

```markdown
## Why this goal exists

_Briefly: the business / engineering driver. Two to four sentences._

## Acceptance criteria (prose)

_Long-form acceptance criteria. Mirrors `acceptance_signals` in params but with rationale,
counterexamples, and what's explicitly out of scope._

## Decomposition rationale

_Why these children, in this archetype mix. When new children are spawned mid-refresh,
the synthesizer appends a one-paragraph rationale here citing the trigger._

## History

_### YYYY-MM-DD — <event>_
```

### prompt_template (the load-bearing artifact)

```
Maintain a goal-tracker. Your job is **aggregation**, not evaluation: read children
via artifact(action="get") and reconcile their state into the goal's params. Do not
recompute children's evidence — trust the child's own params.

INPUTS (gather):
- This goal's current params.
- For each `children[].artifact_id`, the child's params via artifact(action="get").
- Optional: commit log scoped to paths the criterion names (gather_from: git_log).

UPDATE RULES:

1. Reconcile each `children[].status` from the child's actual status, normalizing
   into our enum (pending|active|in-progress|done|blocked|orphan|unknown):
   - failure_table child → "done" if 0 failures, "active" otherwise
   - task_list child → "done" if all tasks done, "in-progress" otherwise
   - metric_baseline child → "done" if current meets the related acceptance_signal, else "in-progress"
   - audit_issues child → "done" if 0 open issues, "active" otherwise
   - reflective child → "done" if child status ∈ {"decided","archived"};
                       "blocked" if "deferred"; "active" otherwise.
   - nested goal child → "done" if child status == "done";
                         "blocked" if ∈ {"blocked","abandoned"};
                         "pending" if "scoping";
                         "in-progress" if "pending-confirmation"; "active" otherwise.
   - Child artifact unreachable → set status: "orphan", DO NOT delete the row.

2. Re-evaluate each `acceptance_signals[].met` from the children's evidence.
   Update the `evidence` string to cite the child id and the specific datum.

3. Append exactly one entry to `progress_log` for this refresh cycle:
   {date: today, note: ≤200-char summary of what changed since previous log,
    evidence_commits: [commits added since last refresh that touched goal paths],
    evidence_artifacts: [child artifact_ids whose status changed]}.
   If nothing changed, append a "no change" entry — never skip the log.

4. AUTO-CLOSE GATE (ALL conditions required):
   a. len(children) > 0
   b. All `children[].status` == "done".
   c. Every `acceptance_signals[].met` is true.
   If all three: set status: "done", append a History entry to body summarizing
   the closing evidence. Otherwise: leave status unchanged.

5. SCOPE GROWTH: if your aggregation surfaces a missing sub-objective (e.g., a new
   test suite must pass, a new metric must be added), you MAY:
   a. Call artifact(action="create", kind="tracker", augment={...}) with the
      appropriate existing archetype (failure_table, task_list, metric_baseline,
      audit_issues, reflective, or nested goal).
   b. Call artifact(action="link", src_id=THIS_GOAL_ID, dst_id=NEW_CHILD_ID, rel="child").
   c. Add the new child to `children[]` with the next free C-N id.
   d. Append one paragraph to body "Decomposition rationale" citing the trigger.

6. NEVER:
   - Delete a child row (use status="orphan" if unreachable).
   - Modify a child's params directly. The child has its own augmentation.
   - Flip status to "done" without satisfying ALL gate conditions including 4a.
   - Append more than one progress_log entry per refresh.

STOP CONDITION (you are done with this refresh when):
- All children reconciled.
- One progress_log entry appended.
- Auto-close gate evaluated.
- Output: the new params object. Body edits only for History append or
  Decomposition rationale append on scope growth.

Body holds rationale and history; params hold mechanical state. Keep them separated.
```

The prompt's most opinionated piece is rule 1's per-archetype status-mapping table. It is the *only* place the goal archetype must know about other archetypes. If a 7th non-goal archetype is added, rule 1 is the single point of edit.

## Surfacing & discoverability

Three layers, each with a named responsibility. Two new affordances; one is just convention.

### S1 — Tag convention + `server_instructions.md` paragraph (agent-agnostic foundation)

Goal-trackers carry `tags: ["goal"]`. The discovery query is:

```
artifact(action="find", kind="tracker",
         filter={"tags":{"in":["goal"]}, "status":{"eq":"active"}})
```

`src/prompts/server_instructions.md` gains one paragraph teaching this query and pointing at `librarian(action="context")` (S3 below) for richer surfacing.

**No code changes** — pure prompt edit. Per the project's "Prompt Surface Consistency" rule, this is a `server_instructions.md` edit and **does not require an `ONBOARDING_VERSION` bump** (server_instructions is loaded fresh per MCP connect).

### S3 — `librarian(action="context")` no-anchor mode prepends "Active goals" header

When `librarian_context` is called with no `anchor_id` and no `topic`, the bundle currently returns a project-wide overview. Extension: before that overview, prepend a `## Active goals in this project` section listing each active goal-tracker (criterion truncated to ~100 chars + status + children counts + artifact_id pointer).

Implementation: one branch in `crates/librarian-mcp/src/tools/context.rs` (or wherever the no-anchor path lives — to verify in writing-plans phase) that runs the S1 discovery query and renders the header before the existing overview body.

**Agent-agnostic.** Every MCP client benefits.

### S5 — Custom `Stop` prompt-hook in `codescout-companion` plugin (Claude Code only)

A prompt-based `Stop` hook registered in `claude-plugins/codescout-companion/` that runs after each assistant turn in a CC session and decides whether to stop the loop based on `goal.status`.

**Why custom Stop hook, not /goal wrapper:** native `/goal` exposes no API to read the active criterion from outside the loop. By owning the Stop hook ourselves, the goal-tracker becomes the single source of truth — full programmatic control, full `--resume` parity, no opaque criterion buffer. The trade-off is losing the native `/goal` overlay panel.

**Hook contract:**

- *Input:* `session_id`, `transcript_path`, `cwd`, `last_assistant_message`.
- *Hook body (prompt template, Haiku 4.5):*
  1. `artifact(find, kind="tracker", tags=["goal"], status="active", scope="project")`
  2. If `N == 0`: `{"continue": true, "reason": "no active goal"}`.
  3. If `N > 1`: `{"continue": true, "reason": "multiple active goals — ambiguous, deferring to default"}`.
  4. If `N == 1`: `artifact(get, id=goal_id)` → params:
     - `status ∈ {"done"}`: `{"continue": false, "reason": "goal done: <criterion>"}`
     - `status ∈ {"blocked","abandoned"}`: `{"continue": false, "reason": "goal <status>: <blocked_reason or criterion>"}`
     - Else: `{"continue": true, "reason_to_continue": "next acceptance signal: <first unmet>"}`
- *Prompt discipline (Hamsa-pinned):* the hook prompt is **status-reader only**, not progress-judge. It does not re-evaluate evidence. It does not flip status to `done`. Refresh is the only writer.
- *Escape hatch:* `.claude/codescout-companion.json` flag `goal_stop_hook: false` disables.
- *MCP unreachable:* fail-open with `{"continue": true, "reason": "codescout MCP unreachable"}` + a one-line warning to `.claude/codescout-companion.log`.

CHANGELOG line 15 (v2.1.143) confirms `Stop` hooks fire after subagent/background-shell completion — our hook inherits this fix automatically.

## Data flow

Four flows. The single-mutator property — **only the refresh flow (3b) writes goal params** — is the central invariant.

### 3a — Goal creation

```
User or agent
  ├─ librarian(action="tracker_design", intent="goal: ...")
  │     ↳ returns the goal archetype (prompt + schema + render_template)
  ├─ artifact(action="create",
  │            kind="tracker",
  │            tags=["goal"],
  │            augment={prompt, params: {criterion, status: "scoping", children: [], ...},
  │                     params_schema, render_template},
  │            body=<from body_skeleton>)
  └─ optional initial decomposition (loop):
        artifact(action="create", kind="tracker", ...)   # any existing archetype
        artifact(action="link", src=goal_id, dst=child_id, rel="child")
        (locally update params.children, then artifact(action="update"))
```

Criterion is written exactly once at create-time. From then on it is read-only.

### 3b — Refresh cycle (the only mutator)

```
trigger (manual via artifact_refresh; periodic via /loop; post-PR)
  ├─ artifact_refresh(action="gather", id=goal_id)
  │     ↳ returns: current params, body excerpt, gather sources
  │       (git_log scoped to criterion paths, each child's current params)
  ├─ LLM synthesis turn (follows the augmentation prompt above)
  │     ↳ reads children's params
  │     ↳ reconciles children[].status per rule 1's mapping
  │     ↳ re-evaluates acceptance_signals[].met
  │     ↳ appends one progress_log entry
  │     ↳ may call artifact(create) + artifact(link) for new children
  │     ↳ may flip status to "done" iff gate (rule 4) passes
  └─ artifact(action="update",
              id=goal_id,
              patch={params: <new>, body: <if scope-growth or auto-close>},
              commit_refresh=true)
```

No new code on this flow — it rides the existing `artifact_refresh` + LLM synthesis + `artifact(update, commit_refresh=true)` pipeline.

### 3c — Stop hook (per CC turn)

```
User invokes claude (interactive | -p | remote control) on a project with an active goal
  ├─ codescout-companion's Stop prompt-hook fires after each assistant turn
  ├─ Hook body (Haiku 4.5):
  │     1. artifact(find, kind="tracker", tags=["goal"], status="active") → N
  │     2. branch on N (see S5 contract above)
  └─ CC main loop receives the verdict
        continue=false → loop stops, user sees "reason"
        continue=true  → next turn fires
```

### 3d — Discovery (S1 + S3, per agent connect)

```
Agent connects to MCP at session start
  ├─ server_instructions.md is loaded fresh → includes S1 paragraph
  ├─ Agent (or skill, or codescout-companion SessionStart hook) calls
  │   librarian(action="context")    # no anchor — S3 extension
  │     ↳ bundle starts with: "## Active goals in this project"
  │       for each: criterion (truncated), status, N/M children done, artifact_id
  └─ Agent has goal context with zero new tools
```

### Scope growth & auto-close (folded into 3b)

These are branches inside the LLM synthesis step of 3b, not separate flows. Highest-stakes branches:

- **Scope growth (rule 5):** synthesizer creates a new child artifact + links it + adds row to `params.children` + appends a rationale paragraph to body.
- **Auto-close (rule 4):** synthesizer flips `status: done` iff ALL gate conditions hold (`len(children) > 0` AND all `children[].status == "done"` AND all `acceptance_signals[].met == true`). Otherwise status is left unchanged — never speculatively flipped.

## Error handling — eight named failure modes

The discipline is **degrade rather than corrupt.** Every failure must leave the goal-tracker in a state the next refresh can recover from.

### 4a — Child artifact unreachable

**Symptom:** `artifact(get, id=child.artifact_id)` returns 404 / archived / wrong-project.

**Recovery:** mark child row `status: "orphan"`. Leave `artifact_id` intact. Append one-line body History entry.

**Escalation:** if all children become orphan, goal auto-flips to `blocked` with `blocked_reason: "no resolvable children"`. Rule 4a (`len(children) > 0`) prevents trivial auto-close on an empty set.

### 4b — Schema validation rejects a merge

**Symptom:** synthesis emits params that violate `params_schema`. Existing `artifact_augment(merge=true)` validation catches this.

**Recovery:** `RecoverableError` returned; params unchanged. Caller retries with corrected payload.

**Escalation:** three consecutive validation failures → log entry in `docs/TODO-tool-misbehaviors.md` (genuine tool failure, not input-driven).

### 4c — LLM hallucinates `status: "done"` without gate passing

**Symptom:** synthesis flips `status: done` despite an unmet acceptance signal or non-done child. Schema validation does NOT catch this (value is in enum and structurally valid).

**Mitigations:**

- *Prompt-level (primary):* rule 4 is phrased as "set status=done IFF all gate conditions hold; otherwise leave status unchanged." Augmentation prompt is the model's first line.
- *Audit-level:* after each refresh, an `artifact_event(kind="verdict", payload={gate_passed: bool, evidence: ...})` is appended. Verdict events are observable outside params; repeated false-done flips become a session entry in `docs/trackers/tool-usage-patterns.md`.
- *Engine-level (deferred):* a `gate_check_status_transitions` flag on `artifact_augment` with a MiniJinja post-condition. Not in v1 — rule of three guard, only one tracker kind needs it.

Hamsa heuristic 8 (same-model self-critique is suspect) is acknowledged. The verdict event log is the cheap second-pass that closes the gap without engine changes.

### 4d — Stop hook can't reach MCP

**Symptom:** hook fires, attempts MCP call, codescout server is down or disconnected.

**Recovery:** **fail-open.** Return `{"continue": true, "reason": "codescout MCP unreachable — falling back to default behavior"}`. Write a one-line warning to `.claude/codescout-companion.log`.

**Rationale:** failing closed would feel like /goal is broken; the hook is an add-on, not a hard dependency. User intent ("loop should be guided by codescout when available") is honored by fail-open + visible log.

### 4e — Multiple active goals in one project

**Symptom:** S1 find query returns `N > 1`.

**Recovery:**

- *Stop hook:* `continue=true, reason="multiple active goals — ambiguous"`. Don't pick one.
- *`librarian_context` S3:* render all in the header, oldest-first by `created_at`.

**Convention enforced at prompt layer:** one active goal per project at a time. No UNIQUE constraint in the database.

### 4f — `disableAllHooks` / `allowManagedHooksOnly` in managed settings

**Symptom:** managed settings prevent our `Stop` hook from registering (per CHANGELOG line 134).

**Recovery:** SessionStart hook in `codescout-companion` introspects managed settings; if disabled, emits one line: `🎯 codescout goal-tracker(s) active, but Stop hook integration is disabled by managed settings — invoke refresh manually.`

**Escalation:** none. Policy, not bug.

### 4g — Stale tracker (never refreshed since creation)

**Symptom:** `artifact_refresh(action="list_stale", scope="project")` shows the goal-tracker beyond `threshold_hours` (default 24).

**Recovery:** caller (periodic /loop, SessionStart hook, or user) sees the stale entry and triggers refresh manually. Standard staleness path; goal-trackers ride existing infrastructure.

**Escalation:** none. Normal idle state.

### 4h — Concurrent refreshes (two agents / two worktrees)

**Symptom:** two agents call `artifact_refresh(gather)` on the same goal concurrently, both synthesize, both `update`.

**Recovery:** last-write-wins by `updated_at`. Loser's mutations are lost; next refresh re-derives. Synthesis is idempotent on stable child inputs → converges within one cycle.

**Escalation:** none in v1. Optimistic concurrency on `artifact(update)` deferred until actual corruption is observed (rule of three).

## Testing strategy

Three tiers. Tiers 1 and 2 are deterministic CI-stable; Tier 3 is the eval gate Hamsa flagged as non-optional.

### Tier 1 — Unit & schema tests

In `crates/librarian-mcp/src/tools/tracker_design.rs` alongside the archetype.

- **T1a — Archetype registration.** Assert `goal` is the 7th entry; assert `len(TRACKER_ARCHETYPES) == 7`.
- **T1b — Archetype self-consistency.** `params_shape_example` validates against `params_schema_example`; `render_template_example` is parseable MiniJinja; `prompt_template` mentions every status enum value.
- **T1c — Schema enforcement on merge.** Three-query sandwich verifying merge rejection leaves params untouched and returns `RecoverableError` (isError=false).
- **T1d — Render template snapshots.** `insta` snapshot tests on the example params → expected markdown.

### Tier 2 — Behavior tests

In `crates/librarian-mcp/tests/goal_archetype.rs` (new file).

- **T2a — Aggregation correctness per child archetype.** One test per child kind (`failure_table`, `task_list`, `metric_baseline`, `audit_issues`, `reflective`, nested `goal`) asserting rule 1's status mapping. Uses a *mocked synthesizer* that follows the prompt rules deterministically.
- **T2b — Three-query sandwich for status-change propagation.** Project canonical pattern:
  1. Create goal G with one `failure_table` child C (3 failures).
  2. Mutate C's params: flip 3 failures to pass. Do NOT refresh G.
  3. Query G's `children[0].status` → assert still `"active"` (stale).
  4. Trigger refresh.
  5. Query G's `children[0].status` → assert `"done"` (fresh).
- **T2c — Scope-growth round-trip.** Synthesis turn that creates + links a new child; assert artifact exists, link edge exists, params.children has C-1 row.
- **T2d — Auto-close gate enforcement.** Two cases: happy path (gate passes → status flips) and forbidden flip (gate fails → status unchanged despite tempting input). Forbidden-flip case runs the real LLM (Haiku 4.5).
- **T2e — Stop hook decision matrix.** Seven branches (status values + N=0/N>1/MCP unreachable) tested as separate `#[test]`.

### Tier 3 — Replay eval (`#[ignore]`-marked, run on demand)

In `crates/librarian-mcp/tests/goal_eval/`.

5 real goals from this repo's recent dev sessions, each hand-constructed at 3 checkpoints (T0 scoping, T1 mid-work, T2 done). Synthesis is run at each checkpoint and scored against a rubric:

| Rubric item | Score | When checked |
|---|---|---|
| correct status | 0/1 | T0, T1, T2 |
| correct evidence citation | 0/1 | T1, T2 |
| no fabrication (no invented children/commits/signals) | 0/1 | T0, T1, T2 |
| appropriate decomposition (right archetype mix) | 0/1 | T0 only |

**Pass criterion:** ≥4 of 5 goals pass all applicable sub-rubrics. Below that, the prompt is the bug — iterate before shipping.

**Goal candidates** (for the spec author to confirm):
- "phase 6 provider lifts ship" (task_list + audit_issues children)
- "retrieval P@5 ≥ 0.20 on benchmark-25tc" (metric_baseline + failure_table children)
- "tools/mod refactor complete" (task_list + audit_issues children)
- "kotlin LSP multiplexer ships" (task_list + reflective + failure_table children)
- "augmentation rollout post-fix audit" (audit_issues + metric_baseline children)

### Not tested (and why)

- *LLM judgment quality at scale.* Covered by in-production `artifact_event(kind="verdict")` log, not by tests.
- *Concurrent refresh races (4h).* Out of scope per design.
- *Status-line / overlay integration.* No public API.
- *Native `/goal` interaction.* Bypassed by design (S5 redesigned).

## Implementation order

Phased so each phase is independently testable and shippable.

### Phase 1 — Archetype only

1. Append `goal` entry to `TRACKER_ARCHETYPES` in `tracker_design.rs`.
2. Add Tier 1 tests (T1a, T1b, T1c, T1d).
3. `cargo fmt && cargo clippy -- -D warnings && cargo test`.

Ships independently. No surfacing, no hook. The archetype is usable via the existing `artifact(create)` + `librarian(tracker_design)` pipeline.

### Phase 2 — Surfacing (S1 + S3)

4. Edit `src/prompts/server_instructions.md` — add one paragraph teaching the discovery query and S3 affordance.
5. Add the no-anchor branch to `librarian(action="context")` that prepends the active-goals header.
6. Add Tier 2 tests T2a (aggregation), T2b (three-query sandwich), T2c (scope growth).
7. Verify against the prompt-surface consistency test (`server::tests::prompt_surfaces_reference_only_real_tools`).

Ships independently. Goal-trackers now discoverable by any MCP client.

### Phase 3 — Stop hook (S5)

8. Author the prompt-based Stop hook in `claude-plugins/codescout-companion/hooks/`.
9. Register the hook in the plugin manifest under the `Stop` event (prompt type).
10. Add the `goal_stop_hook` flag to `.claude/codescout-companion.json` schema with `true` default.
11. Add Tier 2 test T2e (Stop hook decision matrix).
12. Add Tier 2 test T2d (auto-close gate, including the real-LLM forbidden-flip case).

Ships when Phase 2 is in. Claude Code users gain the integration.

### Phase 4 — Eval gate (Tier 3)

13. Construct 5 real-goal fixtures × 3 checkpoints each.
14. Author `eval.rs` harness running synthesis at each checkpoint, scoring against the rubric.
15. Run the eval. If <4/5 goals pass, iterate the augmentation prompt and re-run.
16. Once ≥4/5 passes, mark the spec `status: validated` and merge to master.

Phase 4 is the **gate before promoting to master.** Phases 1–3 may land on `experiments`; cherry-pick to master only after Phase 4 passes.

## Open questions

The following are flagged for resolution during writing-plans, not blocking design approval:

- **Where does the no-anchor `librarian_context` branch live?** `crates/librarian-mcp/src/tools/context.rs` is the suspected file but not verified in this design pass. Confirm during plan writing.
- **Is the `verdict` artifact_event kind already defined?** If not, propose adding to `librarian_event::EventKind` enum during plan writing — small addition, but real.
- **Plugin manifest format for prompt-based Stop hooks.** Confirm against current `codescout-companion` plugin manifest conventions; CHANGELOG line 323 notes `monitors` should go under `experimental` — verify whether prompt-based Stop hooks have an analogous placement.
- **Default model for the Stop hook.** Haiku 4.5 is assumed; confirm against CC defaults at implementation time.
- **Should the spec mandate a CLAUDE.md rule** ("when starting work toward a clear outcome, create a goal-tracker") similar to the bug-tracker rule in `docs/superpowers/specs/2026-05-09-bug-tracker-template-design.md`? Deferred — adopt the convention organically first.

## Validation after implementation

After Phase 4 passes:

- `cargo fmt && cargo clippy -- -D warnings && cargo test` clean.
- Tier 3 eval ≥4/5.
- Manual smoke test in a live MCP session: `/mcp` restart with release build, create a real goal-tracker for a real piece of work, drive a /goal-equivalent loop end-to-end via the Stop hook, verify auto-close behavior.
- Update `CHANGELOG.md` and bump version per the release cycle in `CLAUDE.md`.
