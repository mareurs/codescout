---
title: TimeMachine — librarian-mcp Artifact Timeline + Narrative Graph
status: draft
date: 2026-04-28
author: Marius Ailinca
tags: [librarian-mcp, design, versioning, graph, timemachine]
---

# TimeMachine — librarian-mcp

> Scope: artifact-only TimeMachine (markdown corpus). Pivot to unified
> docs+code KG is tracked, not designed here. See §12.

## 1. Motivation

`librarian-mcp` indexes markdown artifacts (specs, plans, ADRs, status docs,
runbooks) across the workspace. The catalog is a derived index — file
frontmatter is source of truth.

In practice, reality drifts away from the file:

- A "Project Overview" written 2026-03-20 still asserts "Discovery phase
  complete" while a 2026-04-28 status doc shows full sprint plan. The overview
  is silently stale.
- Decisions move through chat / Jira / email / meetings the librarian was not
  part of, so its idea of "current state" lags reality.
- An ADR can be effectively superseded by a newer plan without anyone editing
  the original ADR's frontmatter.
- Spec/plan revisions land without recording **why** the revision was
  attempted, **what the hypothesis was**, or **whether it worked**. The
  artifact has a new state but no narrative around how it got there.

There is no first-class way to say "this artifact's actual state, anchored to
commit X, is Y" — independent of the file content itself — and no way to
record "I am about to revise this because Z, with these inputs". `artifact_observe`
adds free-form notes; `artifact_link rel="supersedes"` flips status. Neither is
queryable as a timeline, neither anchors to git, neither carries the structure
to answer "what was true at commit X" or "why was this change attempted".

This spec adds **two layers** atop the existing artifact graph:

1. A **delta layer** — typed, append-only event log per artifact, anchored to
   git commits, recording state mutations.
2. A **narrative layer** — `intent` events capture hypotheses *before* changes;
   `verdict` events close them *after*; `resolves` edges connect the pair.
   Turns the log from a delta feed into a hypothesis-driven graph.

Plus workspace-level reads that surface freshness drift: "show me every artifact
as it claimed to be at commit X, and how that compares to HEAD."

## 2. Goals

- Record state-change events on artifacts as first-class graph nodes.
- Anchor every event to a git commit (the file's last commit + workspace HEAD
  at recording time).
- Capture hypothesis/verdict pairs as first-class events so timelines explain
  not just what+when but why+outcome.
- Answer time-travel queries: state at commit X, changes between commits A
  and B, what triggered a supersession, which hypotheses are open.
- Surface workspace-wide freshness drift: artifacts whose `freshness_at_as_of`
  diverges from `freshness_now`.
- Surface `freshness` ∈ {fresh, unknown, stale, superseded} on every artifact,
  derived from the event log.
- Stay on SQLite. Reuse existing graph traversal in `artifact_graph` /
  `librarian_context`.

## 3. Non-goals

- File-content versioning. Git already does this.
- Bi-temporal database (no `valid_time` vs `transaction_time` distinction yet).
- Automatic change detection (file watchers, hooks). Events are explicit
  writes via tool calls or by Claude on prompt.
- Hard validation of intent's `inputs` refs. Soft refs only in v1.
- Migration of `artifact_observe` rows into the new events table.
  Observations stay where they are; new code writes to `events`.
- Web UI for timeline visualization. Future work.
- Code-symbol nodes in the graph. Tracked as a pivot (§12), not designed here.

## 4. Schema (SQLite)

Additive. No destructive changes to existing tables.

```sql
CREATE TABLE events (
  id            TEXT PRIMARY KEY,    -- ulid (sortable, time-prefixed)
  artifact_id   TEXT NOT NULL REFERENCES artifacts(id),
  kind          TEXT NOT NULL CHECK (kind IN (
                  'note', 'reviewed', 'status_change', 'field_patch',
                  'superseded_by', 'external_signal',
                  'intent', 'verdict'           -- narrative layer
                )),
  payload       TEXT NOT NULL,        -- JSON, shape per kind (see §5)
  anchor_commit TEXT,                 -- commit hash where artifact_id file lived (NULL = file untracked)
  head_commit   TEXT,                 -- workspace HEAD when event was recorded
  author        TEXT,                 -- 'system' | 'user' | 'claude' | <agent name>
  created_at    INTEGER NOT NULL      -- ms epoch
);
CREATE INDEX events_artifact_idx     ON events(artifact_id, created_at DESC);
CREATE INDEX events_head_commit_idx  ON events(head_commit);
CREATE INDEX events_anchor_commit_idx ON events(anchor_commit);
CREATE INDEX events_kind_idx         ON events(kind);

CREATE TABLE commits (
  hash         TEXT PRIMARY KEY,
  repo         TEXT NOT NULL,
  authored_at  INTEGER,
  subject      TEXT,                  -- first line of message
  topo_order   INTEGER                -- backfilled by reindex; used for "between A and B" range queries
);
CREATE INDEX commits_repo_topo_idx ON commits(repo, topo_order);

CREATE TABLE sources (
  id           TEXT PRIMARY KEY,      -- e.g. 'chat:southpole:msg:12345', 'jira:AUC-78'
  uri          TEXT NOT NULL,
  kind         TEXT NOT NULL CHECK (kind IN (
                  'chat','jira','gmail','confluence','drive','calendar','manual'
                )),
  payload      TEXT,                  -- JSON snapshot (subject, snippet, ts, …)
  ingested_at  INTEGER NOT NULL
);

CREATE TABLE event_edges (
  src_event_id    TEXT NOT NULL REFERENCES events(id),
  dst_event_id    TEXT REFERENCES events(id),
  dst_artifact_id TEXT REFERENCES artifacts(id),
  dst_source_id   TEXT REFERENCES sources(id),
  rel             TEXT NOT NULL CHECK (rel IN (
                    'parent', 'mutates', 'triggered_by', 'merges_with',
                    'resolves'                  -- verdict → intent
                  )),
  PRIMARY KEY (src_event_id, rel,
               COALESCE(dst_event_id, ''),
               COALESCE(dst_artifact_id, ''),
               COALESCE(dst_source_id, ''))
);
CREATE INDEX event_edges_src_idx ON event_edges(src_event_id, rel);
CREATE INDEX event_edges_dst_artifact_idx ON event_edges(dst_artifact_id);
CREATE INDEX event_edges_dst_event_idx    ON event_edges(dst_event_id);
```

### Notes on the graph shape

- The common case is one event mutates one artifact. We denormalize via
  `events.artifact_id` for fast `artifact_timeline` reads. No `mutates` row in
  `event_edges` is needed for the primary artifact.
- Cross-cutting events (e.g. one status doc supersedes overview AND advances a
  plan) get extra `event_edges(rel='mutates', dst_artifact_id=…)` rows.
- Per-artifact chain by default: `parent` edge points to the previous event on
  the same artifact. Cross-cutting events can fan-in (`merges_with`).
- `event_edges.rel='triggered_by'` carries causality from external sources
  (chat / jira / meeting that prompted this event).
- `event_edges.rel='resolves'` carries causality from verdict → intent. Both
  endpoints are events; uses `dst_event_id`. A verdict resolves at most one
  intent; an intent should be resolved at most once (sanity-checked, not
  hard-constrained).

## 5. Payload shapes by kind

| kind | payload | semantic |
|------|---------|----------|
| `note` | `{"text": str}` | free-form observation. Equivalent to `artifact_observe`. |
| `reviewed` | `{"text": str?, "confirms_state": bool}` | freshness ping; the artifact still reflects reality at this commit. |
| `status_change` | `{"from": str?, "to": str}` | frontmatter `status` field changed (also writes to file). |
| `field_patch` | `{"field": str, "from": any?, "to": any}` | any other frontmatter field changed (also writes to file). |
| `superseded_by` | `{"target_artifact_id": str, "reason": str?}` | this artifact is replaced by another. |
| `external_signal` | `{"source_id": str, "summary": str}` | external thing happened that affects this artifact's state but did not yet change the file. |
| `intent` | see below | hypothesis recorded *before* a planned change. |
| `verdict` | see below | outcome recorded *after* the change attempt. |

### `intent` payload

```json
{
  "hypothesis": "free-text I-expect-X-because-Y, required",
  "plan": "optional outline of steps",
  "inputs": [
    { "artifact_id": "...",
      "anchor_commit": "...",
      "note": "what state of this input the hypothesis assumes" }
  ],
  "expected_mutations": ["artifact_id_a", "artifact_id_b"]
}
```

`inputs` are **soft refs** (no FK validation, not promoted to edges in v1).
Pivot to hard `event_edges.rel='depends_on'` is purely additive once drift
becomes a felt problem.

### `verdict` payload

```json
{
  "intent_event_id": "convention; canonical link is the rel='resolves' edge",
  "outcome": "confirmed | refuted | partial | abandoned",
  "summary": "what happened, what was learned",
  "follow_ups": ["free-text or new intent stub ids"]
}
```

`status_change` and `field_patch` round-trip through frontmatter on disk
(preserving "file is source of truth"). `reviewed`, `external_signal`,
`intent`, `verdict` live only in the events table — they describe *narrative
around* file content, not the content itself.

## 6. Tool surface (MCP)

### New tools

#### `artifact_event_create`

```
artifact_event_create(
  artifact_id: str,
  kind: enum,
  payload: object,
  source: {uri, kind, payload?}?,        // creates/refs a source row
  anchor_commit: str? = None,            // default: `git log -1 <file>` of artifact
  head_commit: str? = None,              // default: workspace HEAD
  parent_event_id: str? = None,          // default: latest event on artifact_id
  also_mutates: [artifact_id]? = None,   // cross-cutting fan-out
  resolves_intent_event_id: str? = None  // verdict-only; emits rel='resolves' edge
) -> { event_id, parent_event_id, anchor_commit, head_commit }
```

- Validates payload against the `kind` schema.
- Inserts `events` row.
- Inserts `event_edges` for `parent`, `triggered_by` (if `source` given),
  `mutates` (each `also_mutates`), `resolves` (if `resolves_intent_event_id`).
- For `status_change` / `field_patch`: edits frontmatter on disk first, then
  writes the event.
- For `superseded_by`: also creates / updates an `artifact_link rel='supersedes'`
  edge so existing consumers keep working.
- For `verdict` with `resolves_intent_event_id`: validates the target is an
  `intent` event; refuses if target already has an incoming `resolves` edge.

#### `artifact_timeline`

```
artifact_timeline(
  artifact_id: str,
  since: int? = None,        // ms epoch lower bound (created_at)
  until: int? = None,
  since_commit: str? = None, // alternative: events whose head_commit ≥ this in topo_order
  until_commit: str? = None,
  kinds: [enum]? = None,
  limit: int = 50
) -> [Event]
```

Newest first. Each Event includes resolved `parent_event_id`,
`triggered_by_source`, `mutates_artifacts`, `resolves_intent_id`,
`resolved_by_verdict_id` (edges flattened).

#### `artifact_state_at`

```
artifact_state_at(
  artifact_id: str,
  commit: str? = None,
  timestamp: int? = None       // exactly one of {commit, timestamp}
) -> {
  status, frontmatter, freshness, latest_event, supersession_chain
}
```

Replays events with anchor ≤ target commit (or created_at ≤ timestamp).
Reconstructs from `status_change` / `field_patch` deltas in event payloads.
If there are no `field_patch` events for a field, that field is reported as
"current value" (i.e. file's frontmatter today, with caveat).

#### `workspace_state_at` (NEW)

```
workspace_state_at(
  commit: str? = None,
  timestamp: int? = None,            // exactly one of {commit, timestamp}
  scope: enum = "project",           // reuses existing scope semantics
  kinds: [str]? = None,              // filter artifact kinds (spec, plan, …)
  include_archived: bool = False,
  freshness: [enum]? = None          // filter; default all
) -> {
  as_of: { commit, timestamp },
  scope: { applied, root, … },
  artifacts: [
    { artifact_id, kind, status, frontmatter,
      freshness_at_as_of: enum,
      freshness_now:      enum,      // diff is the staleness signal
      latest_event_at_as_of: { id, kind, created_at },
      supersession_chain: [...] }
  ],
  hints: { hidden_archived, more_in_repo, … }
}
```

Per artifact in scope: run `artifact_state_at` and compute both
`freshness_at_as_of` and `freshness_now`. Cap at exploring-mode 200-row limit;
overflow returns hints.

### Extended tools

- `artifact_get` adds `latest_event` (id, kind, created_at, head_commit) and
  `freshness` (see §7).
- `artifact_graph` adds `include_events: bool = False`. When true, BFS includes
  `event` and `source` nodes via `event_edges` (including `resolves`).
- `librarian_reindex` learns to:
  1. enumerate commits in workspace repos via `git2`,
  2. upsert `commits` rows (`hash`, `repo`, `authored_at`, `subject`),
  3. recompute `topo_order` per repo using `git rev-list --topo-order`.

### Server-instructions delta

Append to librarian-mcp's MCP `instructions` block:

```text
## Event authorship

- Before non-trivial artifact work (revising a spec/plan/ADR, supersession,
  status flip), emit an `intent` event capturing hypothesis + soft `inputs` refs.
- After the work concludes, emit a paired `verdict` event with
  `resolves_intent_event_id` set. Outcome ∈ confirmed|refuted|partial|abandoned.
- After confirming an artifact still reflects reality, emit a `reviewed` event
  (freshness ping). Cheap and high-value.
- Reserve direct user calls for high-stakes events: `superseded_by`,
  `external_signal` (chat/jira/meeting decisions the librarian did not see).
- Do not emit `intent` for trivial mechanical edits (typo fixes, link rot).
  Threshold: would a future reader want to know *why* this changed? If yes, emit.
```

### Kept tools (back-compat)

- `artifact_observe` keeps its current API; under the hood it now writes a row
  to `events` with `kind='note'` AND keeps writing the legacy observation row
  (dual-write) for one release. Removal scheduled for a follow-up spec.
- `artifact_link rel='supersedes'` keeps working; in addition emits a
  `superseded_by` event so timeline + link graph stay consistent.

### Deferred (tracked, not in v1)

- `librarian_context as_of:` — reconstructs frontmatter+freshness annotation at
  the given point when packing the bundle. Body text comes from current disk
  (git already time-travels content). Cheap to add later, low value to ship now.
- Hard `event_edges.rel='depends_on'` for intent inputs.
- Hook-driven auto-emission (git post-commit / file-save).

## 7. Freshness derivation

`freshness(artifact)` ∈ {`fresh`, `unknown`, `stale`, `superseded`}:

```text
if latest_event.kind == 'superseded_by':
    return 'superseded'

newest_reviewed = latest event of kind 'reviewed' on artifact
if newest_reviewed is None:
    return 'unknown'

if file.updated_at > newest_reviewed.created_at:
    return 'stale'        # file changed after last review

if topo_distance(HEAD, newest_reviewed.head_commit) > FRESHNESS_HORIZON:
    return 'stale'        # workspace moved on without re-review

return 'fresh'
```

`FRESHNESS_HORIZON` is configurable per `workspace.toml` (default: 50 commits).

`workspace_state_at` computes this twice per artifact: once with HEAD = `as_of`
commit (yielding `freshness_at_as_of`), once with current HEAD (yielding
`freshness_now`). The diff between the two is the user-facing staleness signal.

This mirrors codescout's `memory_staleness` (anchor sidecars surfaced via
`project_status`), but inverts the storage: events table is source of truth
instead of `.anchors.toml` files.

## 8. Migration

1. Add `events`, `commits`, `sources`, `event_edges` tables. Idempotent — skip
   if present.
2. Backfill commits: enumerate every repo in `workspace.toml`, walk full
   history, upsert `commits`.
3. Backfill events:
   - For each existing artifact, write one synthetic `note` event with
     `payload={"text":"imported"}`, `anchor_commit = file's last commit`,
     `created_at = file's last commit time`, `author='system'`.
   - For each existing `artifact_link(rel='supersedes', src=A, dst=B)`, write a
     synthetic `superseded_by` event on `A` with
     `payload={"target_artifact_id": B}`, anchored to whichever commit
     introduced the supersedes link if discoverable, else file's last commit.
4. No file-content rewrites. No frontmatter changes during migration.
5. The `intent` / `verdict` event kinds and `resolves` edge relation are pure
   CHECK-constraint relaxations — no row migration needed. Existing rows are
   unaffected.

Migration is a single SQL script + a one-shot Rust task triggered by the next
`librarian_reindex` after upgrade. Failures are idempotent (re-runnable).

## 9. Testing

### Unit

- Payload validators per `kind` (rejects bad shapes), including new
  `intent` / `verdict` payloads.
- Freshness derivation rules (table-driven, covers all four return values +
  boundary commits).
- `artifact_state_at` replay logic for {`status_change` only, `field_patch`
  only, mixed, with `superseded_by` mid-stream}.

### Integration

- Round-trip: `event_create(status_change)` → frontmatter on disk updated →
  `timeline` returns event → `state_at` returns new status →
  `artifact_get.freshness` reflects it.
- `librarian_reindex` populates `commits.topo_order` correctly on a fixture
  repo.
- `artifact_graph(include_events=true)` returns the right node/edge counts,
  including `resolves` edges.

### Narrative-layer tests (NEW)

1. **`intent_verdict_round_trip`** — write intent → write verdict with
   `resolves_intent_event_id` → assert `artifact_timeline` returns both, with
   verdict's `resolves_intent_id` flattened from edges.
2. **`open_intents_query`** — write three intents, resolve two, assert the
   open-intents SQL (`events k='intent' WHERE id NOT IN (SELECT dst_event_id
   FROM event_edges WHERE rel='resolves')`) returns the third only.
3. **`verdict_without_intent_is_data_bug`** — sanity-check helper returns the
   orphan verdict; tests assert the helper's correctness.
4. **`workspace_state_at_freshness_diff`** — three artifacts, one stale, one
   fresh, one superseded at HEAD; query at an earlier commit; assert
   `freshness_at_as_of` ≠ `freshness_now` for the staled one.
5. **`intent_inputs_payload_passthrough`** — soft refs round-trip unchanged;
   no FK validation, no pruning of dead refs.

### Sandwich freshness regression (extends project pattern)

Three-query sandwich for `workspace_state_at`:

- Query at commit X → record baseline.
- Append a `reviewed` event with `head_commit > X`, `anchor_commit` at X.
- Query at commit X again → assert *unchanged* (regression bit: the new event
  lives outside the as-of window).
- Query at HEAD → assert freshness moved (new event counts now).

### End-to-end (SP-style fixture)

- Seed: `Project Overview.md` (status=`active`, last commit C1).
- Action: `artifact_event_create(reviewed, …)` at C1.
- Advance commits to C2 (file untouched).
- Action: write `Status Doc V1.md`, then `artifact_event_create(superseded_by,
  target=Status Doc V1)` on Project Overview at C2, with `triggered_by` source
  = a chat message id.
- Assert: `artifact_get(Project Overview).freshness == 'superseded'`;
  `artifact_state_at(Project Overview, commit=C1).freshness == 'fresh'`;
  `artifact_graph(Status Doc V1, include_events=true)` walks back through the
  supersession to the chat source.

## 10. Open questions / deferred

- **Bi-temporal modeling.** Events have one timestamp (`created_at`) and one
  anchor (`head_commit`). Do we need `valid_from`/`valid_until` for retroactive
  corrections? Defer until we see a real need.
- **Authoritative source-table.** `sources` is currently a soft cache.
  Pull-through to live MCPs vs snapshot-only? Defer; start snapshot-only.
- **Auto-event hooks.** Hook on git post-commit / file save to auto-emit
  events? Out of scope; pivot trigger = "we keep forgetting `verdict` events".
- **Subsuming `artifact_observe` fully.** Dual-write transitional; follow-up
  spec removes the legacy observations table.
- **Event compaction.** Append-only forever might bloat over years. Future
  pass could collapse N consecutive `reviewed` events. Not needed at current
  scale.
- **Cross-repo commit topological order.** `topo_order` is per-repo. Cross-repo
  "between A and B" needs timestamp range fallback. Acceptable for v1.
- **Hard `depends_on` edges for intent inputs.** Soft refs only in v1; pivot
  trigger = stale-input drift becomes a felt problem.
- **`librarian_context as_of:` parameter.** Tracked for v1.1.
- **Pivot to unified docs+code KG (scope B).** See §12.

## 11. Rollout

Phased, additive only. No feature flag, no dual-write window beyond
`artifact_observe`'s existing one.

| Phase | Work | Tests |
|-------|------|-------|
| 1 | Schema + write path: CHECK constraint relaxations for `intent`/`verdict`/`resolves`; `artifact_event_create` accepts new kinds + `resolves_intent_event_id` arg | 1, 3, 5 (§9) |
| 2 | Workspace read: `workspace_state_at` tool, scope/filter reuse, freshness-diff output | 2, 4, sandwich (§9) |
| 3 | Server-instructions update + ONBOARDING_VERSION bump (codescout pattern) + tracker artifact + inaugural intent event landed in same commit | manual verify |
| 4 | Tracked-deferred items: `librarian_context as_of:`, hook-driven auto-emission, hard `depends_on` edges. Each gets a one-line entry in §10. | n/a |

Schema + migration + the original three core tools (`artifact_event_create`,
`artifact_timeline`, `artifact_state_at`) ship in Phase 1. `workspace_state_at`
ships in Phase 2. `artifact_get.freshness` + `artifact_graph(include_events)`
+ `librarian_reindex` commit backfill ride along across Phases 1–2 as the
read-side polish lands.

Removal of legacy observation dual-write tracked in a follow-up spec.

## 12. Pivot tracker — artifacts → unified docs+code KG

The "TimeMachine over markdown" scope was chosen to validate the design before
absorbing code-level nodes (codescout symbols, files, commits-as-first-class).
This section codifies how to know when to pivot.

### Tracker artifact

`docs/superpowers/trackers/timemachine-pivot-to-codescout.md`
- librarian artifact kind: `tracker`, status: `active`
- Body: short rationale + the signals table below + chronological observation
  log (each entry an `artifact_observe` / `note` event on this tracker).

### Inaugural intent event (Phase 3 commit)

```
artifact_event_create(
  artifact_id = <tracker.id>,
  kind        = "intent",
  payload = {
    "hypothesis": "Artifact-only TimeMachine (scope A) is sufficient for one
                   quarter. Pivot to unified docs+code KG (scope B) only if
                   specific signals fire.",
    "plan":       "Accumulate observations on the tracker. Re-evaluate at
                   2026-08-01 or when ≥3 high-weight signals fire, whichever
                   comes first.",
    "inputs": [ {"artifact_id": "<this spec>",
                 "anchor_commit": "<landing sha>"} ],
    "expected_mutations": []
  }
)
```

### Pivot signal table

| Signal | Pivot weight |
|---|---|
| Users repeatedly ask "what code existed when this spec was written" | high |
| `mutates` edges frequently point at conceptual code modules with no librarian artifact | high |
| Freshness drifts because code changed but no markdown event captures it | high |
| `external_signal` events outnumber file-change events | medium |
| Workspace `as_of` queries used >2×/week per active project | medium |
| Tracker accumulates >10 "wish I could query code at commit X" observations | medium |

### Verdict

The verdict event resolves the inaugural intent at the re-evaluation date or
trigger:

- `confirmed` → ship A as-is, close the tracker.
- `refuted` → start scope-B design (new spec); link the new spec via
  `follow_ups` in the verdict payload.
- `partial` → narrow extension (e.g. add commits-as-nodes only); spawn a
  smaller follow-up.

This is the design's meta-loop: the spec writes the first intent against
itself.

## 13. Changelog

- 2026-04-28 (initial draft) — schema, delta-layer events, freshness, basic
  tool surface.
- 2026-04-28 (TimeMachine revision) — added narrative layer (`intent` /
  `verdict` events, `resolves` edge, soft `inputs` refs); added
  `workspace_state_at`; deferred `librarian_context as_of:` to v1.1; added
  server-instructions delta; added §12 pivot tracker; restructured §11
  rollout into phases.
