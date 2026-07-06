---
kind: spec
status: draft
title: Atomic index allocation for librarian trackers
owners: []
tags:
  - librarian
  - trackers
topic: atomic-index-allocation
---

# Atomic index allocation for librarian trackers

**Companion spec:** `docs/superpowers/specs/2026-07-06-constitution-tracker-design.md`
— a constitution-type tracker archetype that consumes this primitive for its own
entries. The two designs are independent and can ship in either order, but the
constitution tracker's own append path assumes this one exists.

## Motivation

Cross-repo exploration of `backend-kotlin` (session logs around its `solver-invariants`
tracker) surfaced a recurring bug: an agent appending a new monotonic-ID entry (`SI-N`)
computes "next free index" from a stale source — a plan written earlier, or a
hand-maintained memory snapshot that lagged the live tracker — causing ID collisions.
Three separate incidents document this:

- `room-capacity-halls-session-log.md` F-3/W-3: a concurrent session had already taken
  `SI-11` by the time a second session tried to register it; root cause named
  explicitly as *"the plan reserved the next id at authoring time without re-checking
  at registration time"* — a race between concurrent sessions sharing one checkout.
- `stage2-availability-session-log.md` F-1: the session-start memory snapshot
  enumerated `SI-1`…`SI-13`; the live tracker had already reached `SI-18`. Root cause:
  *"the `solver-invariants` codescout memory is a hand-maintained summary that lags
  the augmented tracker."*
- `archive/planning-solution-copy-session-log.md` W-2: an approved plan baked in
  `SI-14`, which collided with `SI-14`/`15`/`16` added earlier the same day.

The same failure mode exists in codescout's own tracker family: `docs/trackers/skill-
frictions.md` has F-005 through F-010 duplicated across two different `##` sections,
non-monotonically ordered within one of them. Even codescout's most mature built-in
tracker archetype, `failure_table`, pushes the same burden onto the agent — its
`prompt_template` reads *"Add new F-N entries for new failures (next free integer)"*,
with no mechanical allocation behind it.

The common root cause across every incident: **the next-index computation was cached**
(in a plan, or in a stale memory summary) instead of read live, at write time, from the
registry itself — worsened by concurrent multi-instance sessions racing the same read.

## Design

### New MCP action — `artifact(action="append_entry")`

Callable wherever `artifact` is called today:

```
artifact(action="append_entry", id="<tracker_id>", entry_collection="failures",
         id_prefix="F", entry={status:"fail", owner:"@x", notes:"..."})
→ { "id": "F-13", "entries": [...whole updated array...] }
```

Server-side, in one SQLite transaction:

1. Load the artifact row and its augmentation.
2. Read `params.<entry_collection>`.
3. Regex the existing ids matching `^<id_prefix>-(\d+)$`; take max+1.
4. Merge the new object (with the computed `id` attached) into the array.
5. Write back, commit.

Because the read-max-write happens inside a single DB transaction, a concurrent
second call blocks until the first commits and then correctly computes `F-14` — this
directly kills the concurrent-write race (F-3/W-3). Because the id is always computed
from the live params array, never a cached snapshot, it also kills the staleness bug
(F-1/W-2).

### Error handling

- Missing or unknown `entry_collection` → `RecoverableError` naming the valid
  collections declared on the artifact's augmentation.
- `id_prefix` that doesn't match the collection's `params_schema` id pattern (e.g.
  schema requires `^F-\d+$` but the caller passes `id_prefix="X"`) → `RecoverableError`
  raised before the transaction opens; never a partial write.

## Testing

- Rust integration test: spawn N concurrent `append_entry` calls against one tracker;
  assert N unique, sequential ids with no gaps or duplicates. This directly models the
  F-3/W-3 concurrent-session race.
- Unit test: `id_prefix`/schema mismatch fails before any write (params unchanged).
- Unit test: regex correctly finds max across non-contiguous ids (e.g. existing
  `F-1, F-3, F-9` → next is `F-10`, not `F-2`).

## Non-goals

- **Prose-only trackers are not touched.** codescout's own F-N/W-N session-log family
  (and U-N, H-N, R-N) have no `entry_collection` — they need the existing
  `docs/conventions/retrofitting-trackers-for-filtering.md` procedure first. Retrofitting
  them is separate, later work, not assumed here.
- **No server-computed `auto_id` for `body_edits`** (regex-scanning markdown headings
  directly, for trackers that stay prose-only). Considered and deferred — heading-regex
  matching is fuzzier than reading a structured array (template placeholders, nested
  subsections, a reused prefix in prose could confuse the scan) and shouldn't be built
  speculatively before this primitive proves out in production use.
