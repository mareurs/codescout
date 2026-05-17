---
title: Goal-Tracker Archetype — Amendment 1
date: 2026-05-17
amends: docs/superpowers/specs/2026-05-16-goal-tracker-design.md
status: draft
---

# Goal-Tracker Archetype — Amendment 1 (2026-05-17)

This amendment records decisions on behavior left undefined by the
original design ([2026-05-16-goal-tracker-design.md](2026-05-16-goal-tracker-design.md)),
surfaced by:

- **Hamsa prompt audit** (2026-05-17 session) — 9 high/med findings against the
  augmentation prompt and discovery surfaces.
- **Lion architecture review** (2026-05-17 session, two passes) — findings on
  synth-mechanism placement, container-pattern naming, and four cross-feature
  issues surfaced when comparing against `audit_doc_refs` and `researcher_tracker`.
- **Refactoring-Yak refactor stress-test** (2026-05-17 session) — three seams
  blocking the proposed Rust-kernel extraction (I1).

The original spec defines *what* a goal-tracker is. This amendment decides
*what predicates and structural guarantees the system must enforce* before the
Rust-kernel refactor (I1) can land cleanly.

**Scope:** decisions only. Implementation belongs in the plan doc
(`docs/superpowers/plans/2026-05-17-i1-refactor.md`, follow-up).

---

## D1 — Empty `task_list` child → `Pending`, not `Done`

**Context:** Hamsa finding H-4. The original prompt's rule 1 maps
`task_list child → "done" if all tasks done, "in-progress" otherwise`. Empty
`tasks` array trivially satisfies "all tasks done" (vacuous truth in any
language with `.all()` semantics). A child created but not yet populated
would auto-close the goal.

**Options considered:**

| Option | Behavior | Verdict |
|---|---|---|
| A. `Done` (status quo, vacuous) | Empty list = goal-close trigger | **Rejected.** Bug-by-omission. Lets a brand-new uninitialized child auto-close a goal. |
| B. `InProgress` | Empty list = work nominally underway | **Rejected.** Lies about progress. No tasks declared ≠ work underway. |
| C. `Pending` | Empty list = no work declared yet | **Adopted.** Honest pre-work signal. Goal auto-close gate (rule 4a `len(children) > 0`) still blocks; this just prevents the vacuous-truth bypass. |
| D. `Unknown` | Empty list = degraded state | Rejected. `Unknown` is reserved for genuinely degraded reads (missing fields, parse errors). Empty is not degraded; it is uninitialized. |

**Decision:** `task_list_status({"tasks": []}) → ChildStatus::Pending`.

**Rust shape:**

```rust
fn task_list_status(params: &Value) -> ChildStatus {
    let tasks = params.get("tasks").and_then(|v| v.as_array());
    match tasks {
        None => ChildStatus::Unknown,           // schema violation
        Some(t) if t.is_empty() => ChildStatus::Pending,   // D1
        Some(t) if t.iter().all(|task| task.get("status").and_then(|s| s.as_str()) == Some("done")) => ChildStatus::Done,
        Some(_) => ChildStatus::InProgress,
    }
}
```

**Confidence:** high.

---

## D2 — `failure_table` flaky/wontfix semantics

**Context:** `failure_table` status enum is `{"fail", "pass", "flaky", "wontfix"}`.
The original prompt's rule 1 says *"done if 0 failures, active otherwise"* without
deciding whether `flaky` or `wontfix` count as failures for goal-closure purposes.

**Decisions:**

- `wontfix` **does not** block goal closure. Rationale: `wontfix` is
  accepted technical debt — explicitly human-acknowledged as non-blocking.
  Including it would force the user to either delete `wontfix` entries
  (loses audit trail) or never close the goal.
- `flaky` **does** block goal closure. Rationale: flaky tests indicate a
  real bug that the suite intermittently surfaces. Marking a goal "done"
  because flaky tests *sometimes* pass is a lie. Flaky requires human
  triage to either fix or downgrade to `wontfix`.

**Predicate:**

```rust
fn failure_table_status(params: &Value) -> ChildStatus {
    let failures = params.get("failures").and_then(|v| v.as_array());
    match failures {
        None => ChildStatus::Unknown,
        Some(f) if f.is_empty() => ChildStatus::Pending,   // mirrors D1
        Some(f) if f.iter().all(|entry| {
            matches!(
                entry.get("status").and_then(|s| s.as_str()),
                Some("pass") | Some("wontfix")
            )
        }) => ChildStatus::Done,
        Some(_) => ChildStatus::Active,
    }
}
```

**Confidence:** medium-high. The flaky decision is the contestable one — some
teams treat flaky as accepted-noise. We treat it as blocking because in a
codescout context, flaky almost always means "real bug, low repro rate." If a
project wants flaky-as-noise semantics, they should mark the entry `wontfix`
with a `notes` line explaining the noise tolerance.

---

## D3 — `deployment_state` child → done iff all listed envs enabled

**Context:** The original spec's rule 1 lists six archetypes
(`failure_table`, `task_list`, `metric_baseline`, `audit_issues`, `reflective`,
nested `goal`) and **omits** `deployment_state` (Hamsa H-1, Lion confirmed).
A `deployment_state` child today maps to `ChildStatus::Unknown` by default —
the goal cannot auto-close even if the rollout is complete.

**Schema reminder** (from `archetype_deployment_state` in `tracker_design.rs`):

```json
{ "flag_name": "...", "envs": { "<env>": { "enabled": bool, "since": str|null } } }
```

**Options considered:**

| Option | Predicate | Verdict |
|---|---|---|
| A. Done iff `envs.prod.enabled == true` | "Prod is the goal" | **Rejected.** Assumes the existence of a `prod` env. Some flags only roll to staging; some have `canary`, `eu-west`, etc. Hard-coding `prod` is incorrect for the general case. |
| B. Done iff all listed envs `enabled: true` | "Full rollout" | **Adopted.** Honors the user's declaration: they listed N envs, the rollout is done when all N are on. If they want partial rollout to count as done, they leave the unintended envs out of the params. |
| C. Done iff goal's `acceptance_signals` mention specific envs and those envs enabled | "Cross-cutting context" | **Deferred.** Requires the same structured `acceptance_signals` schema as `metric_baseline` (see D4). Until D4 ships, the freeform signal path handles this case. |

**Decision:** Option B.

**Predicate:**

```rust
fn deployment_state_status(params: &Value) -> ChildStatus {
    let envs = params.get("envs").and_then(|v| v.as_object());
    match envs {
        None => ChildStatus::Unknown,
        Some(e) if e.is_empty() => ChildStatus::Pending,
        Some(e) if e.values().all(|env| {
            env.get("enabled").and_then(|b| b.as_bool()) == Some(true)
        }) => ChildStatus::Done,
        Some(_) => ChildStatus::InProgress,
    }
}
```

**Empty-envs case:** `Pending`, mirroring D1. A `deployment_state` with
no envs declared is uninitialized, not vacuously-rolled-out.

**Confidence:** medium. The choice between B and C is empirical — we'll learn
which is needed once goal-trackers wrap real rollouts. Until then, B is the
honest default.

---

## D4 — Structured `acceptance_signals` schema

**Context:** Yak refactor seam S2. The original spec's `acceptance_signals`
are freeform: `{description, met, evidence}` — all strings/booleans. This
shape forces the LLM to do all signal evaluation (since the `met` boolean
has no derivation rule). It also makes `metric_baseline` aggregation
impossible without freeform NL parsing.

**Decision:** Add an optional `kind` discriminant. `kind: "freeform"` is the
default and preserves backward compatibility. Other kinds carry kind-specific
fields whose semantics are fully Rust-evaluable.

**New schema (additive — existing trackers continue to work):**

```json
{
  "acceptance_signals": [
    {
      "description": "P@5 ≥ 0.20 on benchmark-25tc",
      "met": false,
      "evidence": "metric_baseline child C-1 current=0.193",
      "kind": "metric_threshold",
      "evidence_child_id": "C-1",
      "metric_key": "P@5",
      "op": ">=",
      "threshold": 0.20
    },
    {
      "description": "no stale code refs in docs/specs/**",
      "met": false,
      "evidence": "audit_issues child C-2 has 3 open",
      "kind": "audit_issues_open_count",
      "evidence_child_id": "C-2",
      "max_open": 0
    },
    {
      "description": "team agrees on V2 architecture",
      "met": false,
      "evidence": "see reflective child C-3 §Decision",
      "kind": "reflective_decided",
      "evidence_child_id": "C-3"
    },
    {
      "description": "no regressions in user feel",
      "met": true,
      "evidence": "manual eyeball check, looks fine",
      "kind": "freeform"
    }
  ]
}
```

### Signal kinds

| `kind` | Required extra fields | Predicate (Rust) | Notes |
|---|---|---|---|
| `freeform` (default) | none | **LLM-evaluated.** Rust passes through. | Backward-compatible. |
| `audit_issues_open_count` | `evidence_child_id`, `max_open` (default 0) | `cited_child.params.issues.iter().filter(status=="open").count() <= max_open` | |
| `failure_table_clean` | `evidence_child_id` | `cited_child.params.failures.iter().all(status in {pass, wontfix})` | Reuses D2's flaky-blocks rule. |
| `task_list_complete` | `evidence_child_id` | `len(cited_child.params.tasks) > 0 && all(status == "done")` | Mirrors D1. |
| `metric_threshold` | `evidence_child_id`, `metric_key`, `op ∈ {>=, >, <=, <, ==}`, `threshold: f64` | `child.params.current[metric_key] op threshold` | Unlocks `metric_baseline` aggregation (Yak S2). |
| `reflective_decided` | `evidence_child_id` | `cited_child.params.status in {"decided", "archived"}` | |
| `deployment_envs_enabled` | `evidence_child_id`, `envs: [str]` (optional, defaults to all) | If `envs` set: all named envs `enabled: true`. Else: same as D3. | Resolves D3 Option C deferral. |

### Schema-level rules

1. `kind` is optional. Missing → treated as `freeform`.
2. `evidence_child_id` is required iff `kind != "freeform"`. Must reference an
   `id` in this goal's `children[]`. Validation error at refresh if missing
   or unresolvable.
3. `description` remains required and human-readable for **all** kinds —
   it is the surface the Stop hook renders ("next acceptance signal: ...").
4. `met` is still authoritative on the wire. Rust kernel **overwrites**
   `met` for non-freeform kinds during refresh; for `freeform` kinds, the
   LLM writes it and Rust passes through.
5. `evidence` remains a freeform string. For non-freeform kinds, the Rust
   kernel rewrites it to a citation template:
   `"<kind>: <child_id> <key>=<observed> vs <op> <threshold>"`.

**Validation:** `params_schema` extended to validate per-kind required fields.
JSON Schema `oneOf` on `kind` enum.

**Confidence:** high on the schema shape, medium on the kinds list — we may
discover `failure_table_clean` and `audit_issues_open_count` need a
"max_severity" or "since_date" parameter once real goals start using them.
Add when pressure is named, per rule of three.

---

## D5 — `progress_log` splits into `refresh_meta` (Rust) + `progress_log[].note` (LLM)

**Context:** Lion finding F-A. The original `progress_log[]` entry mixes
deterministic data (`date`, `evidence_commits`, `evidence_artifacts`) with
LLM narration (`note`). Every refresh involves the LLM even on no-change
cycles. The prompt's rule 3 ("never skip the log") forces noise.

**Decision:** Split into two fields.

```json
{
  "refresh_meta": {
    "last_refresh_at": "2026-05-17T08:00:00Z",
    "last_refresh_commit": "abc12345",
    "children_status_delta": [
      { "child_id": "C-1", "from": "active", "to": "done" }
    ],
    "commit_count_since_last": 3,
    "unchanged_refreshes": 0,
    "degraded": false,
    "orphan_children": []
  },
  "progress_log": [
    {
      "date": "2026-05-14",
      "note": "chat-eval-v3 stable. Need final 7pt P@5.",
      "evidence_commits": ["abc1234"],
      "evidence_artifacts": ["d4e5f6a7"]
    }
  ]
}
```

**Ownership:**

- `refresh_meta` is **Rust-only**. Overwritten verbatim on every refresh.
- `progress_log[]` is **LLM-only**. Append-only. Entry appended *only*
  when `refresh_meta.children_status_delta` is non-empty OR
  `refresh_meta.commit_count_since_last > 0` AND the LLM has something
  substantive to record. "No change" refreshes increment
  `refresh_meta.unchanged_refreshes` instead of appending a log entry.

**Why this matters:**

1. **Stop hook gains staleness signal.** `refresh_meta.last_refresh_at`
   is what the Stop hook reads to emit `"params last refreshed: <ts>"`
   in `reason_to_continue`. Resolves Hamsa S-1.
2. **Idempotency.** `refresh_meta` is byte-identical on no-change refreshes
   (modulo `last_refresh_at` which Rust controls). `progress_log` does not
   grow on no-change refreshes. Matches audit-doc-refs's idempotency contract.
3. **`progress_log` stops being log spam.** Long-running goals don't accrete
   a "no change" entry per session.

**Migration:** existing trackers have `progress_log[]` only. First refresh
after the schema upgrade backfills `refresh_meta` from the most recent
`progress_log[]` entry (best-effort: copy `date` to `last_refresh_at`,
empty delta, zero counts).

**Confidence:** high.

---

## D6 — Rule 6 NEVER list converts to Rust guarantees

**Context:** Lion finding F-B. The original prompt's rule 6 enumerates six
things the LLM must not do. A NEVER list is the prompt confessing the
architecture cannot prevent those mistakes structurally.

**Decision:** Each NEVER bullet becomes a Rust-level guarantee. The augmentation
prompt's rule 6 reduces to zero bullets after I1+I9 land. The list below is
also the implementation checklist.

| Original NEVER bullet | Becomes |
|---|---|
| "Delete a child row" | Merger function takes `&[Child]` immutably; rows addressed by primary key; merge logic explicitly carries forward unmatched prior rows. |
| "Modify a child's params directly" | Goal merge function takes `child_params: &Value` (read-only borrow). No mutable borrow. |
| "Flip status to 'done' without satisfying ALL gate conditions" | `evaluate_gate(params) -> GateOutcome` is the only path that returns `Status::Done`. Rule 4 disappears from the prompt. |
| "Append more than one progress_log entry per refresh" | Rust appends 0 or 1 entries based on delta detection (D5). Prompt cannot append directly. |
| "Trust the child's own params" | Implicit: Rust kernel reads child params verbatim, never re-derives. |
| "Flip status to 'done' without satisfying 4a (`len(children) > 0`)" | Subsumed by `evaluate_gate`. |

**Side effect:** the prompt loses its negation-heavy tone. Hamsa-cleaner.

**Confidence:** high.

---

## D7 — `audit_issues` archetype schema generalization

**Context:** Lion finding F-D. The published archetype schema for
`audit_issues` (in `tracker_design.rs::archetype_audit_issues`, lines 155-189)
requires only `[n, title, severity, status]` (+ optional `owner`). The runtime
shape produced by `audit_doc_refs` carries 13 fields per issue (`severity_reason`,
`ref_kind`, `md_file`, `md_line`, `raw_ref`, `first_seen_commit`,
`first_seen_at`, `last_verified_at`, plus a flattened `extra` for
forward-compat).

The mismatch is non-fatal today because `additionalProperties` is unset and
JSON Schema defaults to permissive. But:

1. Any future tightening (`additionalProperties: false`) breaks `audit_doc_refs`.
2. `params_schema_example` lies about the actual contract.
3. A hand-written `audit_issues` tracker has no way to know about the richer
   fields the renderer might expect.

**Decision:** Generalize the canonical archetype schema to **acknowledge** the
optional fields without **requiring** them.

**Updated schema** (deltas only; full schema in implementation):

```json
{
  "type": "object",
  "required": ["issues"],
  "properties": {
    "issues": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["n", "title", "severity", "status"],
        "properties": {
          "n":        { "type": "integer", "minimum": 1 },
          "title":    { "type": "string" },
          "severity": { "type": "string", "enum": ["high", "med", "low"] },
          "severity_reason": { "type": "string" },
          "status":   { "type": "string", "enum": ["open", "in-progress", "fixed", "wontfix"] },
          "owner":    { "type": "string" },
          "ref_kind": { "type": "string" },
          "md_file":  { "type": "string" },
          "md_line":  { "type": "integer" },
          "raw_ref":  { "type": "string" },
          "first_seen_commit": { "type": "string" },
          "first_seen_at":     { "type": "string", "format": "date-time" },
          "last_verified_at":  { "type": "string", "format": "date-time" }
        }
      }
    },
    "scan_meta": { "type": "object" },
    "parse_warnings": { "type": "array" }
  }
}
```

**No new archetype.** `audit_doc_refs` keeps writing to `audit_issues`; the
mismatch is closed by widening the canonical schema, not by inventing
`doc_audit`. Rationale: rule of three — one auditing feature exists today
(audit-doc-refs); inventing a second archetype before the second feature
ships is premature multiplication (Lion's standing objection to
sibling-archetype proliferation, original spec Option B rejection).

**Confidence:** medium-high. The widening is safe. The "no new archetype"
call should be revisited if a second auditing feature (e.g., link-rot
crawler) lands with materially different fields.

---

## D8 — `metric_baseline` aggregation now possible

**Context:** Yak refactor seam S2. With D4 in place, `metric_baseline` is no
longer an unaggregatable cross-cutting case.

**Decision:** `metric_baseline` participates in the Rust kernel via the
**signal-side**, not the **child-side**. Specifically:

- `child_status_pure("metric_baseline", child_params)` still returns
  `ChildStatus::Unknown` — there is no archetype-local "is this metric
  good enough?" predicate, because the threshold lives on the goal's
  acceptance signal, not on the metric_baseline child itself.
- Instead, when an `acceptance_signal` has `kind: "metric_threshold"`,
  the kernel reads `child.params.current[metric_key]` and evaluates
  the comparison. This is the cross-cutting evaluation D4 enables.
- The metric_baseline child's overall `status` in `children[]` is
  derived from the **set of metric_threshold signals that cite it**:
  - All citing signals met → `Done`.
  - Some met, some unmet → `InProgress`.
  - None met → `Active`.
  - No citing signals → `Active` (the child exists but is not tied to
    a closure condition; goal will not auto-close on it).

**Predicate:**

```rust
fn metric_baseline_status_in_context(
    child_id: &str,
    parent_signals: &[AcceptanceSignal],
) -> ChildStatus {
    let citing: Vec<&AcceptanceSignal> = parent_signals.iter()
        .filter(|s| s.kind() == SignalKind::MetricThreshold
                 && s.evidence_child_id() == Some(child_id))
        .collect();
    match citing.as_slice() {
        []                                       => ChildStatus::Active,
        s if s.iter().all(|sig| sig.met)         => ChildStatus::Done,
        s if s.iter().any(|sig| sig.met)         => ChildStatus::InProgress,
        _                                        => ChildStatus::Active,
    }
}
```

This is the only archetype whose status needs parent-context. It belongs in
a `goal_aggregation::contextual::*` submodule, separate from the
context-free `child_status_pure` for the other six archetypes.

**Confidence:** high.

---

## D9 — Children-free goal-trackers (Hamsa H-3, Lion follow-up)

**Context:** The original gate rule 4a requires `len(children) > 0` for
auto-close. The `params_schema_example` allows `children: []`. A
one-criterion-direct-check goal (no decomposition) trips the gate forever.

**Options considered:**

| Option | Behavior | Verdict |
|---|---|---|
| A. Widen the gate | `len(children) > 0 OR (len(signals) > 0 AND all met with non-empty evidence)` | Rejected. Adds complexity; `freeform` signals can hallucinate "met:true" trivially. |
| B. Narrow `when_to_use` | Mandate decomposition; redirect non-decomposable cases to `metric_baseline` / `task_list` / etc. directly | **Adopted.** Cleaner architecture, matches the container-pattern spirit. |
| C. Add a `single_signal_mode` flag to the archetype | Per-tracker opt-in to children-free closure | Rejected. New flag for a case better served by a different archetype. |

**Decision:** Narrow `when_to_use`. The archetype's `when_to_use` field is
amended to read:

> Use only when the work decomposes into 2+ typed sub-trackers. For a
> goal with no decomposition (e.g., one metric, one task list), use the
> underlying archetype directly. The container archetype's job is
> aggregation; without children to aggregate, it is the wrong shape.

The `params_schema` is **not** changed to add `minItems: 2` — the boundary
is enforced socially via `when_to_use`, not structurally. Reason: this lets a
goal start at `len(children) == 1` during scoping (the user has identified
the first sub-objective but not yet the rest) without schema rejection.
Closure remains gated at 2+ children via runtime check.

**Implementation:** rule 4a stays `len(children) > 0`. A separate rule 4d is
added: `len(children) >= 2`. Auto-close requires both. A `len(children) == 1`
goal is structurally valid (scoping phase) but cannot auto-close until a
second child is added or the goal is converted.

**Confidence:** medium. We may discover one-child goals are common enough
to warrant Option A after all. Revisit after first 10 real goals.

---

## D10 — `scope growth` cap (Hamsa H-6)

**Context:** The original prompt's rule 5 allows the synthesizer to spawn
new children mid-refresh if a missing sub-objective is surfaced. No cap.
No quarantine. No "newly-spawned" flag.

**Decision:** Cap at **1 new child per refresh cycle**. Multi-gap refreshes
defer additional children to subsequent cycles, logging the deferred items
in `refresh_meta.deferred_decompositions: [str]`.

**Rationale:**

- Bounds blast radius of a runaway model (the Hamsa concern).
- Forces the model to prioritize: which gap is most urgent.
- Lets the next refresh re-evaluate whether the first child resolved the
  apparent gap before spawning a second.
- Cheap to implement: count `artifact(action="create")` calls during the
  refresh; reject the 2nd+.

**Prompt-side guidance:** rule 5 is updated to read:

> If your aggregation surfaces multiple missing sub-objectives,
> create at most ONE new child this refresh. List the remaining gaps in
> `refresh_meta.deferred_decompositions` for the next cycle to evaluate.

**Confidence:** high.

---

## D11 — Verdict-event audit mechanism (resolves §4c)

**Context:** The original spec §4c proposes appending an
`artifact_event(kind="verdict", payload={gate_passed: bool, evidence: ...})`
after each refresh as a hallucination audit. Lion finding A-3 showed the
`verdict` event kind already exists with `outcome ∈ {confirmed, refuted, partial, abandoned}`
and **requires** `resolves_intent_event_id`. The spec's proposed payload
would fail validation and populate `orphan_verdicts()`.

**Options considered:**

| Option | Shape | Verdict |
|---|---|---|
| A. Pair `intent` + `verdict` events per refresh | Refresh emits `intent("evaluate gate")` → `verdict(outcome="confirmed"/"refuted", resolves_intent_event_id=...)` | Rejected. Heavyweight; doubles event volume per refresh; verbose. |
| B. Use `note` or `field_patch` event with `gate_check` tag in payload | One event per refresh, lighter shape | **Adopted.** Composes with existing event kinds; no schema gymnastics. |
| C. Skip events entirely; rely on `refresh_meta.gate_outcome` | Audit lives in params, not events | Rejected. Loses time-series queryability; events are the right place for "what happened when". |
| D. Add new `gate_check` event kind | New schema | Rejected. Premature; rule of three on event kinds, not yet met. |

**Decision:** Option B. After each refresh, emit:

```rust
artifact_event::create(EventKind::Note, json!({
    "tag": "gate_check",
    "gate_passed": <bool>,
    "evidence": {
        "children_all_done": <bool>,
        "signals_all_met": <bool>,
        "children_count": <usize>,
        "signal_count_met": <usize>,
        "signal_count_total": <usize>,
    },
    "refresh_meta_at": "<ISO8601>",
}))
```

Events are queryable via `artifact_event::list(artifact_id, kinds=["note"])`,
filterable by `payload.tag == "gate_check"` for the audit trail.

**Confidence:** high.

---

## Summary table

| ID | Decision | Implements which finding | Blocks I1 step |
|---|---|---|---|
| D1 | Empty `task_list` → `Pending` | Hamsa H-4 | 2 |
| D2 | `wontfix` passes, `flaky` blocks | (refactor-time decision) | 2 |
| D3 | `deployment_state` done iff all envs enabled | Hamsa H-1 | 2 |
| D4 | Structured `acceptance_signals` with `kind` | Yak S2 | 6, 7 |
| D5 | `refresh_meta` (Rust) + `progress_log[].note` (LLM) | Lion F-A, Hamsa S-1 | 3 |
| D6 | NEVER list → Rust guarantees | Lion F-B | 10 (post-I1) |
| D7 | `audit_issues` schema widened | Lion F-D | independent |
| D8 | `metric_baseline` aggregates via signals, not status | Yak S2 | 7 |
| D9 | `goal` requires 2+ children for closure | Hamsa H-3 | independent |
| D10 | Scope growth capped at 1 new child/refresh | Hamsa H-6 | independent |
| D11 | Gate-check via `note` event, not `verdict` | Lion A-3 | independent |

## What is NOT decided here (deferred to plan or future spec)

- **CLAUDE.md rule for goal-tracker creation** (Hamsa F1 / open Q5).
  Punted to convention-builds-organically per the original spec.
- **Stop hook → CLI subcommand** (Lion Q4, I9). Architecture decision,
  separate refactor doc.
- **`TrackerMerger` trait extraction** (I8). Explicitly rejected at this
  scale; revisit when a second concrete merger half-exists.
- **Tracker `managed_by` flag** (I7). Defer until two features actually
  collide on a tracker.

## References

- Original design: [2026-05-16-goal-tracker-design.md](2026-05-16-goal-tracker-design.md)
- Audit-doc-refs design (sibling feature): [2026-05-16-audit-doc-refs-design.md](2026-05-16-audit-doc-refs-design.md)
- Researcher-tracker design (3rd-concrete in synth-mechanism comparison): [2026-05-08-researcher-tracker-design.md](2026-05-08-researcher-tracker-design.md)
- Implementation plan (forthcoming): `docs/superpowers/plans/2026-05-17-i1-refactor.md`
