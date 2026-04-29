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
`target event not found`, etc.) returns `anyhow::anyhow!`, which surfaces as
`isError: true` to the MCP caller. Project convention (CLAUDE.md): expected
input failures should be `RecoverableError`-style so sibling tool calls in a
batched MCP request survive.

**Fix shape:** introduce/reuse a `RecoverableError` wrapper from
`crates/librarian-mcp/src/tools/mod.rs` (or local helper) and route the
input-validation arms through it. Keep `anyhow!` for genuine bugs only.

**Estimate:** 1 short session. Touch one file. Tests already cover the error
strings — adjust assertions if the error type changes.

### 2. Transaction-wrap event + edges insert

**Where:** `crates/librarian-mcp/src/tools/event_create.rs::call`.

**Issue:** Catalog mutex is acquired/dropped multiple times across a single
`call`: pre-flight intent check, parent-event lookup, frontmatter write,
event insert, source upsert, edges insert. No transaction spans them. A
crash or interleaving writer between the event insert and the edges insert
leaves an event without its edges (parent, mutates, triggered_by, resolves).

**Fix shape:** wrap the row + edges inserts in
`cat.unchecked_transaction()`; commit after edges land. Keep the frontmatter
write before the transaction (file-system effect can't roll back).

**Estimate:** 1 short session. Same file. Add a test that simulates a panic
between event-insert and edge-insert (via a feature flag or direct catalog
manipulation) — assert no orphan rows.

### 3. `apply_payload_to_frontmatter` runs outside the catalog lock

**Where:** `crates/librarian-mcp/src/tools/event_create.rs`.

**Issue:** Frontmatter write happens before the lock is taken. Two
concurrent `status_change` callers on the same artifact can interleave file
writes in arbitrary order. Probability low (single-user MCP) but spec said
"edits frontmatter on disk *first*, then writes the event" implying a single
ordered atomic unit.

**Fix shape:** option A — take a per-artifact-id mutex (via `Arc<Mutex<()>>`
keyed by artifact_id) around frontmatter write + event insert. Option B —
serialize all writes through a single coarse lock (simpler, lower throughput,
fine at v1 scale).

**Estimate:** 1 short session. Touches event_create + a small registry
struct.

### 4. `write_field_to_frontmatter` silently skips unknown fields

**Where:** `crates/librarian-mcp/src/tools/update.rs` (extracted helper).

**Issue:** Frontmatter struct uses `#[serde(deny_unknown_fields)]` with a
fixed set of scalar fields (`status`, `title`, `topic`, `time_scope`). A
`field_patch` event for `owners`, `tags`, `confidence`, etc. writes the
event row but **silently** does nothing on disk. Caller has no idea the
write was a no-op.

**Fix shape:** either (a) return an error from `write_field_to_frontmatter`
when the field is not writable, propagated up to `event_create`'s caller —
event row is NOT written in that case; (b) extend the helper to handle
array fields (`owners`, `tags`).

**Estimate:** 1 short session. Recommendation: (a) for v1.1; (b) when a
real `field_patch` for owners/tags is needed.

### 5. Missing narrative-layer tests (spec §9 #2, #3, #5)

**Where:** new test cases needed; locations would be in
`crates/librarian-mcp/src/catalog/events.rs::tests` and adjacent.

**Issues:**
- `open_intents_query` — no test asserts the
  `events k='intent' WHERE id NOT IN (... resolves edges)` SQL.
- `verdict_without_intent_is_data_bug` — no helper or test checks for
  orphan verdicts (verdict events with no `resolves` edge).
- `intent_inputs_payload_passthrough` — no test asserts that the soft
  `inputs` refs round-trip unchanged through write→read.

**Fix shape:** three new unit tests. Open-intents query lives in catalog;
either add a `pub fn open_intents(cat) -> Result<Vec<EventRow>>` and test
it, or assert via raw SQL in the test. Orphan-verdict helper similarly.
Inputs-passthrough is one round-trip via `event_create` + read-back from
`events.payload`.

**Estimate:** 1 short session. ~30 lines of test code.

### 6. Frontmatter hydration duplicated `state_at` ↔ `get.rs`

**Where:** `crates/librarian-mcp/src/tools/state_at.rs::replay_state_at`
seeds a frontmatter map from `ArtifactRow`; `crates/librarian-mcp/src/tools/get.rs`
does similar work. Field list will drift.

**Fix shape:** extract `pub(crate) fn build_frontmatter_map(art: &ArtifactRow) -> Map<String, Value>`
in `catalog::artifact` (or a new `catalog::frontmatter` module). Call from
both consumers.

**Estimate:** 1 short session. Mechanical refactor.

### 7. `replay_state_at` post-fetch filter on `created_at <= cutoff`

**Where:** `crates/librarian-mcp/src/tools/state_at.rs::replay_state_at`.

**Issue:** Currently calls `events::timeline_for_artifact(_, _, None, usize::MAX)`
then filters in Rust by `created_at <= cutoff_ts`. Wasteful for artifacts
with hundreds of events.

**Fix shape:** extend `events::timeline_for_artifact` to accept an optional
`until` (already added in fix #2 / commit `4988e52`!). Switch `replay_state_at`
to pass `until=Some(cutoff_ts)`.

**Estimate:** 5-line edit. Actually small enough to combine with another
item or land standalone.

### 8. State_at vs workspace_state_at freshness-field-name asymmetry

**Where:** `crates/librarian-mcp/src/tools/state_at.rs` returns
`"freshness"` (scalar); `workspace_state_at.rs` returns
`"freshness_at_as_of"` (+ `"freshness_now"` per-artifact).

**Issue:** By design (workspace surfaces both at-as-of + now-diff;
state_at is single-artifact so the bare label suffices), but it can
confuse callers switching between the two tools.

**Fix shape:** option A — document the asymmetry in tool descriptions.
Option B — make state_at also return `freshness_at_as_of` + `freshness_now`
for consistency. (B is a breaking response-shape change.)

**Estimate:** documentation-only fix is 1 short session. Schema change is
breaking and should wait for v2.

### 9. Timeline ordering within same millisecond

**Where:** `crates/librarian-mcp/src/catalog/events.rs::timeline_for_artifact`
and friends.

**Issue:** `ORDER BY created_at DESC, id DESC` — id is ULID, lexicographic.
Within the same ms, ULID's random bits dominate, so ordering is not
strictly creation-order. Smoke test surfaced this when looking up events
by array position; fix was to key by known event id.

**Fix shape:** likely **defer** — ULIDs are time-ordered to ms resolution
which matches `created_at`'s precision. Within-ms collisions are rare in
real workflows. Document the behavior in tool descriptions if/when callers
hit it.

**Estimate:** 0 (document-only) for v1.x; consider a monotonic counter
column post v2.

## Resolved during review

These were called out in the code review but turned out to be false alarms
on closer inspection (recorded so we don't keep re-litigating):

- **1.6 — `workspace_state_at` response shape drift.** Smoke test
  (`crates/librarian-mcp/tests/timemachine_smoke.rs`, commit `452843f`)
  confirmed actual fields (`freshness_at_as_of`, `freshness_now`,
  `status_at_as_of`, `freshness_changed`, `latest_event_at_as_of`)
  match the implementation exactly. No drift to fix.

## Re-eval

Re-read this list at the start of each follow-up TimeMachine session.
Prioritize 1, 2, 5 (correctness + safety + test coverage) over 3, 4
(quality-of-life). 6, 7, 8, 9 are nice-to-have.
