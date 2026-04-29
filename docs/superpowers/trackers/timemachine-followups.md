---
title: TimeMachine v1 — should-fix-soon backlog
status: active
date: 2026-04-29
kind: tracker
tags: [librarian-mcp, timemachine, backlog]
---

# TimeMachine v1 — should-fix-soon backlog

Follow-up items raised by code review (commit `4da1d2f` spec, `b0533d9`
plan, `06a7e55..d08e90a` impl, `452843f` smoke). Each is bounded, post-master
acceptable, and tracked here so it doesn't leak. Spec §10 holds **deferred**
items (out of v1 scope by design); this file holds **fixable** items.

References:
- Spec: `docs/superpowers/specs/2026-04-28-librarian-timeline-design.md`
- Plan: `docs/superpowers/plans/2026-04-29-librarian-timemachine.md`
- Code-review report: in conversation transcript (final dispatch on
  `experiments`, 2026-04-29)

## Items

### 1. `RecoverableError` vs `anyhow!` in `event_create` input validation

**Where:** `crates/librarian-mcp/src/tools/event_create.rs::call`,
`validate_payload`.

**Issue:** Every input-driven validation error (`unknown event kind`,
`note.text required`, `intent.hypothesis required`, `intent already resolved`,
`target event not found`, etc.) returned `anyhow::anyhow!`, surfaced as
`isError: true` to the MCP caller. Sibling tool calls aborted.

**Resolution:** introduced `RecoverableError` in
`crates/librarian-mcp/src/tools/mod.rs`. Routing in
`server.rs::map_tool_result` downcasts and serialises the error as a
success body `{"error": ..., "hint": ...}` so sibling parallel calls
survive. All input-validation arms in `event_create.rs` now use
`RecoverableError::new` / `with_hint`.

**Status:** DONE (phase 1).
### 2. Transaction-wrap event + edges insert

**Where:** `crates/librarian-mcp/src/tools/event_create.rs::call`.

**Issue:** Catalog mutex was acquired/dropped multiple times. No
transaction spanned event + edges. A crash between them left an event
without its edges.

**Resolution:** added `_with` / `_in_tx` helpers in `catalog::events`,
`catalog::sources`, `catalog::links`, `catalog::event_edges` so callers
can share a `rusqlite::Transaction`. `event_create::call` now opens one
`unchecked_transaction()`, writes source row + supersedes link + event
row + edges, commits as a unit. Frontmatter mutation stays before the
transaction (filesystem effect can't roll back). Test
`rollback_on_failure_after_event_insert_leaves_no_orphan_row` uses a
thread-local injection flag to abort the call between event insert and
edge insert; asserts both `events` and `event_edges` are empty after.

**Status:** DONE (phase 1).
### 3. `apply_payload_to_frontmatter` runs outside the catalog lock

**Where:** `crates/librarian-mcp/src/tools/event_create.rs`.

**Issue:** Frontmatter write happened before the lock was taken. Two
concurrent `status_change` callers on the same artifact could interleave
file writes in arbitrary order. `latest_for_artifact` could also race so
both calls saw the same parent and the resulting graph fanned instead
of chained.

**Resolution:** added a per-artifact `WriteLockRegistry` (a static
`OnceLock<HashMap<String, Arc<tokio::sync::Mutex<()>>>>` in
`event_create.rs`). `event_create::call` acquires the per-artifact-id
lock immediately after payload validation and holds it for the whole
function — spanning frontmatter mutation, parent-event lookup, and the
row + edges transaction. Calls on different artifacts do not contend.
Test `concurrent_calls_on_same_artifact_chain_not_fan` spawns two
concurrent calls on a multi-thread runtime and asserts the resulting
parent edges form a chain (one event with `parent_event_id=None`, the
other pointing at it) instead of a fan.

**Status:** DONE (phase 2).
### 4. `write_field_to_frontmatter` silently skips unknown fields

**Where:** `crates/librarian-mcp/src/tools/update.rs`.

**Issue:** Frontmatter struct uses a fixed set of scalar fields
(`status`, `title`, `topic`, `time_scope`). A `field_patch` event for
`owners`, `tags`, `confidence`, etc. wrote the event row but silently
did nothing on disk — caller had no idea the write was a no-op.

**Resolution:** option (a) from the original fix shape.
`write_field_to_frontmatter` now returns a `RecoverableError` when the
field is not in the writable allow-list. Because
`event_create::call` invokes the helper before opening the database
transaction, the error propagates up and the event row is *not*
written. Test
`field_patch_unwritable_field_errors_and_writes_no_event` asserts both
the error type and that `events.count == 0` after a `field_patch` for
`owners`.

Option (b) — extending the helper to also write array fields
(`owners`, `tags`) — still deferred until a real `field_patch` for
those fields is needed.

**Status:** DONE (phase 2).
### 5. Missing narrative-layer tests (spec §9 #2, #3, #5)

**Where:** `crates/librarian-mcp/src/catalog/events.rs::tests`,
`crates/librarian-mcp/src/tools/event_create.rs::tests`.

**Resolution:**
- Added `events::open_intents(cat) -> Vec<EventRow>` + test
  `open_intents_excludes_resolved_and_includes_unresolved`.
- Added `events::orphan_verdicts(cat) -> Vec<EventRow>` + test
  `verdict_without_intent_is_data_bug`.
- Added `intent_inputs_payload_passthrough` round-trip test in
  `event_create.rs::tests`.

**Status:** DONE (phase 1).
### 6. Frontmatter hydration duplicated `state_at` ↔ `get.rs`

**Where:** `crates/librarian-mcp/src/catalog/artifact.rs`,
`crates/librarian-mcp/src/tools/state_at.rs`.

**Issue (re-evaluated):** the duplication originally claimed in this
item turned out to be one-sided — `tools/get.rs` parses frontmatter
from disk via `frontmatter::parse(content)` and does *not* hydrate from
`ArtifactRow` the way `state_at::replay_state_at` did. Only one
in-memory hydration path existed, so there was no immediate drift to
fix. Refactor done anyway because the helper makes the schema
authoritative in one place for any future caller (e.g. a per-artifact
timeline view) and shrinks `replay_state_at`.

**Resolution:** added
`crate::catalog::artifact::build_frontmatter_map(art) -> Map<String, Value>`
and switched `replay_state_at` to call it. Field list (status, title,
kind, tags, owners, topic, time_scope) lives in one place.

**Status:** DONE (phase 3).
### 7. `replay_state_at` post-fetch filter on `created_at <= cutoff`

**Where:** `crates/librarian-mcp/src/tools/state_at.rs::replay_state_at`.

**Issue:** `replay_state_at` previously called
`events::timeline_for_artifact(_, _, None, None, usize::MAX)` and
filtered in Rust by `created_at <= cutoff_ts`. Wasteful for artifacts
with hundreds of events.

**Resolution:** switched to
`events::timeline_for_artifact(_, _, None, Some(cutoff_ts), usize::MAX)`
so the cutoff is pushed into SQL. The `until` parameter was added in
commit `4988e52` (the SQL push-down for the live `timeline` tool); this
item was just the consumer-side switch.

**Status:** DONE (phase 3).
### 8. State_at vs workspace_state_at freshness-field-name asymmetry

**Where:** `crates/librarian-mcp/src/tools/state_at.rs`,
`crates/librarian-mcp/src/tools/workspace_state_at.rs`.

**Issue:** `state_at` returns `"freshness"`; `workspace_state_at`
returns `"freshness_at_as_of"` + `"freshness_now"`. Asymmetric by
design (workspace surfaces both views, state_at single-artifact uses
the bare label) but caller-confusing.

**Resolution:** documentation-only fix per the original recommendation
(option A). Both tool descriptions now cross-reference each other and
name the asymmetric fields explicitly. No schema change — a breaking
response-shape unification stays parked for v2.

**Status:** DONE (phase 3, doc-only).
### 9. Timeline ordering within same millisecond

**Where:** `crates/librarian-mcp/src/catalog/events.rs::timeline_for_artifact`,
`crates/librarian-mcp/src/tools/timeline.rs`.

**Issue:** `ORDER BY created_at DESC, id DESC` — id is ULID,
lexicographic. Within the same millisecond, ULID's random bits
dominate, so ordering is not strictly creation-order. Surfaced by the
smoke test when looking up events by array position.

**Resolution:** documentation-only fix per the original recommendation.
`artifact_timeline`'s tool description now spells out the
`created_at DESC, id DESC` ordering rule, calls out that within-ms
ordering may not match strict creation order, and tells callers to pin
lookups by event id rather than array position. A monotonic counter
column remains a post-v2 consideration.

**Status:** DONE (phase 3, doc-only).
## Resolved during review

These were called out in the code review but turned out to be false alarms
on closer inspection (recorded so we don't keep re-litigating):

- **1.6 — `workspace_state_at` response shape drift.** Smoke test
  (`crates/librarian-mcp/tests/timemachine_smoke.rs`, commit `452843f`)
  confirmed actual fields (`freshness_at_as_of`, `freshness_now`,
  `status_at_as_of`, `freshness_changed`, `latest_event_at_as_of`)
  match the implementation exactly. No drift to fix.

## Re-eval

All items resolved as of phase 3. Tracker is now archival — keep for
reference but no further work expected. If new TimeMachine follow-ups
emerge from later sessions, open a fresh tracker rather than
reviving this one.
