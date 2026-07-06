---
kind: spec
status: draft
title: Constitution tracker — mechanically-enforced, context-conditional rules
owners: []
tags:
  - librarian
  - codescout-companion
  - trackers
topic: constitution-tracker
---

# Constitution tracker — mechanically-enforced, context-conditional rules

**Companion spec:** `docs/superpowers/specs/2026-07-06-librarian-atomic-index-allocation-design.md`
— this design's own rule entries are appended through that primitive, so a
constitution tracker never suffers the collision bug it exists to prevent elsewhere.
The two designs are independent otherwise and can ship in either order.

## Motivation

`backend-kotlin`'s `solver-invariants` tracker is a good reference for *data shape*: a
librarian augmented artifact with tagged, filterable entries (`params.invariants`,
`entry_collection: "invariants"`) plus full prose per entry in the body. But its
context-conditional injection — "read this before touching solver constraint code" —
is pure prose trust, expressed only as a CLAUDE.md instruction. There is no hook or
mechanical enforcement; a passive staleness check (`anchors.toml` content hashes) exists
but only surfaces drift if an agent happens to check `workspace(status)` — it already
failed to prevent a real incident (the F-1 stale-memory-snapshot bug documented in the
companion index-allocation spec).

codescout's 7 existing tracker archetypes (`deployment_state`, `failure_table`,
`metric_baseline`, `audit_issues`, `task_list`, `reflective`, `goal`) are all shape
templates for *what a tracker's data looks like*. None of them push content into an
agent's context based on what it's about to touch. The only existing "automatic
surfacing" mechanism, `librarian(context, topic=...)`, is pulled on demand by
topic/anchor search — never pushed by tool-call target. This spec adds an 8th
archetype, `constitution`, plus the enforcement mechanism it needs to actually be
"must follow no matter what" rather than "must follow, we hope."

## Why prose-trust injection isn't enough

Checked whether the existing `hookify` plugin already solves this (it has `file_path` +
`regex_match` conditions and a `message` body — close to what's needed). Its two
actions are `warn` (returns a bare `systemMessage`, shown to the **human** in the
terminal, never fed back into the model's context) and `block` (a generic deny with
combined rule messages, no per-rule severity/versioning/evidence trail, and rules live
one-per-file rather than in a queryable, cross-linked registry). Only a `deny` decision's
`permissionDecisionReason` reliably reaches the **agent**, because the tool call fails
and the reason string comes back as content the model reads — this was observed
directly during this design's own research session (a denied `Bash` call changed the
next tool the model chose). Extending hookify to add that path, plus the librarian's
filtering/versioning/evidence trail, would mean forking it in practice, not reusing it.
This design instead treats the librarian tracker as the source of truth and adds a new,
narrow enforcement hook.

## Design

### Data model — 8th archetype, `constitution`

```
params.rules = [{
  id: "C-1",
  paths: ["**/solver/**", "**/*Constraint*.kt"],   // optional — absent means global
  title: "...",
  rule: "<short imperative>",
  status: "active" | "superseded",
}]
entry_collection: "rules"
```

Body holds full prose per `## C-N` section (why / how to apply / evidence) — the same
shape as `failure_table` / `solver-invariants`. New rules are added via the companion
spec's `artifact(action="append_entry")`.

**Single-tier enforcement.** Every entry is enforced identically. There is no
"must" vs. "should" split — an advisory-only rule doesn't belong in a constitution
tracker; it belongs in a regular tracker or memory, where being read is best-effort by
design.

### `paths`: path-scoped vs. global rules

- **Path-scoped** (`paths` present) — enforced by tool-call targeting (Channel 1).
- **Global** (`paths` absent) — always relevant regardless of what's being touched
  (e.g. "never commit secrets"). Enforced by session-level injection (Channel 2), not
  by a `paths: ["**"]` glob hack — absence of `paths` is a deliberate, distinct
  semantic, not a wildcard match.

### Enforcement channel 1 — `PreToolUse` hook, path-scoped rules

New codescout-companion hook, registered for `edit_code`, `edit_file`, `create_file`,
`Edit`, `Write` (covers both codescout-governed and ungoverned projects). On a matching
call:

1. Extract the target path from `tool_input`.
2. Shell into a new, read-only `codescout constitution-check --path <path>` CLI
   subcommand — queries the catalog for active `constitution` trackers, glob-matches
   `paths` against the given path, returns matching rules as JSON. Fast, synchronous,
   no MCP round-trip.
3. Collect every matching rule not yet seen this epoch (see Compaction safety below).
   If any, return `hookSpecificOutput: {permissionDecision: "deny",
   permissionDecisionReason: <all unseen rules' prose, concatenated>}`. Mark all
   surfaced rules seen for this epoch.
4. If every matching rule was already seen this epoch, return no decision (allow
   silently) — the goal is "read it once before touching this," not "can never touch
   this file again."

**Batching:** multiple rules matching one call are combined into a single deny, not
denied one at a time — one correction round-trip, not N.

### Enforcement channel 2 — `UserPromptSubmit` hook, global rules

Global (path-less) rules are surfaced via a `UserPromptSubmit` hook, which supports
injecting `additionalContext` (unlike `PreToolUse`'s non-blocking path). Gated by an
"already surfaced this epoch" flag per session: fires once after session start, and
again after each compaction bumps the epoch — not on every turn. Multiple global rules
are combined into one digest block, same batching principle as Channel 1.

### Compaction safety — the epoch counter

Both channels key their "seen" state as `(session_id, epoch, rule_id)`, not
`(session_id, rule_id)`. A new codescout-companion `PreCompact` hook increments the
session's epoch on every compaction. Because "seen" is scoped to the current epoch,
every rule looks unseen again immediately after a compaction — the next matching tool
call (Channel 1) or the next user turn (Channel 2) re-surfaces it through the channels
above. This is deliberately not built on `PreCompact` injecting content itself — its
exact payload/capabilities should be verified during implementation rather than
assumed; the design instead relies on re-arming the two channels already known to work.

**State storage:** a small local state file (e.g. `.codescout/constitution-seen.json`),
keyed by session id, holding the current epoch and the set of `(epoch, rule_id)` pairs
already surfaced.

### Error handling

Hook failures never block the tool — mirrors the project's existing companion hooks
and hookify's own `finally: sys.exit(0)` pattern. A broken `constitution-check`
degrades to "no injection," never to "everything blocked."

## Testing

- Unit tests for the glob-matcher (paths in/out of pattern; path-less rules never
  matched by the path-scoped matcher).
- State-file tests: seen/unseen transition within one epoch; epoch bump clears prior
  "seen" entries.
- Hook-level test: fresh session + matching path → `permissionDecision: deny`; repeat
  call, same epoch → no decision (allow); call after a simulated epoch bump → `deny`
  again.
- Batching test: two rules matching one call → one combined deny, not two.

## Non-goals for this round

- No UI/dashboard for constitution rules.
- No auto-migration of `backend-kotlin`'s existing `solver-invariants` tracker into the
  new `constitution` archetype — a candidate follow-up once this proves out here.
- No "should"-tier (advisory-only) constitution entries.

## Open items to verify during implementation

- Exact `PreCompact` hook payload/capabilities in the current Claude Code version —
  confirm it does not need to inject content directly (this design doesn't depend on
  it doing so, but the epoch-increment call site should be re-checked against it).
- Whether `UserPromptSubmit`'s `additionalContext` support has any size/rate limits
  relevant to a large global-rules digest.
