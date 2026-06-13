# Design — Dzo Legibility Probes & Honest usage.db Logging

**Date:** 2026-06-13
**Status:** approved (design); awaiting implementation plan
**Author:** Dzo (legibility-dzo) + Marius
**Topic:** Deterministic legibility probe + structured friction logging, feeding a
self-reconciling, importance-sorted refactor backlog.

## Problem

Picking refactor targets for machine-legibility today is a hand-rolled archaeology
dig: raw SQL over `.codescout/usage.db`, fighting an `overflowed` column that is
structurally always `0`, reverse-engineering truncation from `output_json LIKE
'%truncat%'`, and manually existence-checking targets because the recorder is
cross-project contaminated (a mirela `CalendarService` phantom surfaced in a
codescout survey — see `docs/trackers/dzo-legibility-session-log.md` F-1, and
`docs/trackers/reconnaissance-patterns.md` R-29).

We want: (1) honest logging that makes friction queryable, and (2) a probe assigned
to the Dzo that inspects code, finds legibility frictions and refactor candidates,
and writes them to a tracker **sorted by importance** that **stays current as
refactoring lands**.

## Decisions log (the five pivots)

| # | Decision | Choice |
|---|---|---|
| 1 | Probe engine | **Hybrid** — code finds candidates mechanically, the Dzo triages the top-N (drops tool-class noise, ranks). Separate from `pika_observations`. |
| 2 | Tracker update | **Auto-reconcile by re-measurement** — re-running the probe re-measures every open candidate; metric-moved → auto-close with before→after delta. |
| 3 | Importance | **Structural defect gates entry, observed cost ranks** — two tiers: biting-now (has recorder friction) above latent (over budget, not yet fetched). |
| 4 | Logging scope | **Fix `overflowed` + structured friction capture** (`overflow_target`, `overflow_tokens`, `err_family`). |
| 5 | Probe home | **`librarian(action="legibility_scan")`** — native in-process MCP action; reuses the librarian artifact machinery for reconcile. |

### Relationship to `pika_observations`

`pika_observations` (shipped 2026-05-17) judges the **caller** — is the agent using
tools well? (Iron-Law slips, misusage), is **LLM-judged**, and writes **only on
explicit ask**. The Dzo probe judges the **callee** — is the *code* legible to the
tools? — and most of its heuristics are **mechanically computable** (body span vs
budget, file symbol-count, truncation events). Different axis, different mechanism.
They are deliberately kept separate; both read `tool_calls`.

## Architecture

```
DELIVERABLE 1: honest logging (src/usage/, src/tools/core/types.rs)
  every tool call → record_content → tool_calls row
    now writes: overflowed (fixed), overflow_target, overflow_tokens,
                err_family, project_root
        │ rolling 30-day evidence
        ▼
DELIVERABLE 2: librarian(action="legibility_scan")
  RECORDER LANE (biting-now)            INDEX LANE (latent)
  SQL over structured fields,           walk project symbol index:
  WHERE project_root = <repo>           bodies > budget, files > symbol-cap,
  → friction per overflow_target        ambiguous name_paths
                └────────┬───────────────────┘
                         ▼
        SCORER: structural gate → observed-cost rank → 2 tiers
                         ▼
        RECONCILER: upsert by candidate key into…
        ▼
THE TRACKER: docs/trackers/legibility-backlog.md (augmented artifact)
  params ← probe-owned: measurements, score, tier, status, before/after delta
  body   ← Dzo-owned:   triage verdict (code-class vs tool-class), human-cost, move
        ▲
        └── Dzo triage pass reads params, writes body, annotates tool-class noise
```

**The loop:** `legibility_scan` → `params` reconciled → Dzo triages top-N into
`body` → one move, green commit → `legibility_scan` re-runs → auto-closes candidates
whose metric moved. The probe is **idempotent**; the Dzo's `body` judgment **survives
re-runs** because the probe only ever touches `params`. This params/body split is the
existing `tool-usage-patterns.md` augmented-artifact pattern.

## Deliverable 1 — Honest logging

Three (plus one) changes, all grounded in current code.

### (a) Fix the dead `overflowed` column

`classify_content_result` (`src/usage/mod.rs:97`) sets `overflowed=true` only when the
result JSON has a top-level key literally named `"overflow"` — a key the real overflow
envelope never emits. The real marker is `output_id` (the buffer handle), built at
`src/tools/core/types.rs:568`.

```rust
// before:  if v.get("overflow").is_some()      ← never fires → always 0
// after:   if v.get("output_id").is_some()     ← the real buffer-handle marker
```

### (b) New structured columns on `tool_calls`

`src/usage/db.rs` schema + `write_record` signature + `write_content` population.

| Column | Type | Source | Always captured? |
|---|---|---|---|
| `overflow_target` | TEXT | symbol/path the call addressed, extracted from input at write time | **yes** (not debug-gated) |
| `overflow_tokens` | INTEGER | `json_len / 4`, exposed on the envelope as `buffered_bytes` at `types.rs:568` | yes, when overflowed |
| `err_family` | TEXT | normalized tag from `error_msg` (`ast_extent_fail`, `ambiguous_name_path`, `lsp_disconnect`, `replace_dropped_sibling`, …) | yes, on error |
| `project_root` | TEXT | the agent's project root (the F-1 contamination fix; enables `WHERE project_root = <repo>`) | **yes** |

`overflow_tokens` requires the overflow envelope to carry the size — a one-field
addition (`"buffered_bytes": json_len`) at `types.rs:568` that the recorder reads.
`overflow_target` must be extracted **always**, not via full `input_json` (which is
debug-gated — `record_content_no_input_in_normal_mode` proves it is `None` in normal
mode). `err_family` is a pure function over `error_msg`; a column (not read-time
computation) makes the probe a plain `GROUP BY` and lets Pika reuse it.

### (c) Migration

Additive, nullable, defaulted columns (`ALTER TABLE … ADD COLUMN`). Old rows keep
working; the rolling 30-day window refills with rich data within a week. **No backfill.**
Retention (30 days) and the debug-gating of full `input_json`/`output_json` are
**unchanged** — the friction fields are small and always-on; bulky payloads stay gated.

## Deliverable 2 — `librarian(action="legibility_scan")`

### Params

- `project` — defaults to active workspace; scopes the recorder lane by `project_root`.
- `write` — default `true` (reconcile the tracker); `false` = dry-run, return ranked JSON.
- `limit` — cap candidates returned/written.

### Recorder lane (biting-now evidence)

`WHERE project_root = <repo>` (F-1 fix), `GROUP BY overflow_target`: truncation count,
retry chains (same input, same session), edit-fail counts by `err_family`. Rolling
30-day window.

### Index lane (latent / structural)

Pure symbol-index walk, no usage.db needed:
- every function/method: `body_tokens > MAX_INLINE_TOKENS` (2,500; read from
  `src/tools/core/types.rs` — single source of truth) → `over_budget_body`.
- every file: `symbols > ~100` or very large → `un_mappable_file`.
- ambiguous `name_path` (matches > 1 symbol) → `name_collision`.

### Scorer

- **Entry gate:** a candidate MUST have a structural defect (`over_budget_body` |
  `un_mappable_file` | `name_collision`). No structural defect → not a candidate. This
  gate mechanically drops tool-class noise before triage (an LSP disconnect on a
  40-line function has no structural defect, so it never enters).
- **Tier 1 (biting-now):** structural defect AND recorder friction > 0 → rank by
  observed cost (the `score` below).
- **Tier 2 (latent):** structural defect AND friction == 0 → rank by structural
  magnitude (`tokens − budget`, or `symbols − cap` for un-mappable files).
- **`score` (default, tunable — a single constant table in the probe):**
  `score = 3·truncations + 2·retries + 2·code_class_edit_fails + 1·other_friction`,
  tie-broken by structural magnitude (`tokens − budget`). `code_class_edit_fails`
  counts only `err_family` values that indicate a code/extractor-shape problem
  (`ast_extent_fail`, `ambiguous_name_path`, `replace_dropped_sibling`); infra families
  (`lsp_disconnect`, `lsp_index_locked`, `mux_startup_fail`) are excluded from the
  score — they are tool-class and must not inflate a code candidate's importance.
  The weights live in one place so the Dzo can justify and adjust them; they are not
  scattered magic numbers.
- **`un_mappable_file` threshold:** the existing symbols-overview overflow threshold
  (the same cap `symbols(path)` uses before it buffers), not a fresh magic number —
  read from the same source as the overview path.
- **Candidate key:** `<rel_file>::<name_path>` (stable as line numbers shift); for an
  un-mappable file, `<rel_file>::(file)`.

## The tracker — `docs/trackers/legibility-backlog.md`

A librarian augmented artifact (the `tool-usage-patterns.md` pattern; id allocated on
first scan). `params` probe-owned, `body` Dzo-owned, same candidate key joins them.

### `params` row (machine)

```json
{ "key": "src/lsp/manager.rs::LspManager/get_or_start",
  "defect": "over_budget_body", "tier": 1, "status": "open",
  "measure": {"tokens": 4180, "budget": 2500, "lines": 242},
  "cost": {"truncations": 14, "edit_fails": 1, "sessions": 2}, "score": 44,
  "first_seen": "2026-06-13", "before": {"tokens": 4180},
  "after": null, "closed_at": null }
```

### `body` section (Dzo, heading = key)

```markdown
## src/lsp/manager.rs::LspManager/get_or_start
**Verdict:** code-class (real over-budget body + ambiguous_name_path collision)
**Move:** extract circuit-breaker + cold-start gates at their seams.
**Human-cost:** none — natural seams.
```

### Reconcile (each scan; probe reads → recomputes → writes `params` via `artifact_augment(merge=true)`; never touches `body`)

- **open, defect still fires** → update `measure`/`cost`/`score`; keep `status:open`;
  preserve `first_seen`/`before`.
- **open, defect gone** (tokens < budget, file now maps) → `status:closed`,
  `after:<new measure>`, `closed_at:today`. **Auto-close with delta.**
- **new key** → insert `status:open`, tier by friction, `before:<current measure>`.
- closed rows stay in `params` for history; render below open ones.

`merge=true` always (never `merge=false` — the documented foot-gun); the probe owns
and fully recomputes the candidates array each run, preserving any other `params` keys.
The librarian renders the sorted table (tier 1 by score, then tier 2) at the top; the
Dzo's `body` prose is the evidence layer beneath.

## Dzo triage workflow

```
1. legibility_scan        → params reconciled (Dzo, or human via /mcp)
2. Dzo triage pass        → top-N tier-1: classify code-class vs tool-class
                            (err_family + Dzo heuristics); write body verdict,
                            the move, human-cost; annotate tool-class "not a refactor"
3. one move, green commit → the Yak's safety net (behavior preserved, tests green)
4. legibility_scan re-run → auto-close with before→after delta
```

The probe automates the Dzo's Phase-1 survey; triage stays the Dzo's judgment. **On-demand
only** — no auto-run per session (mirrors Pika's "write on ask"; avoids churn). A one-line
pointer in the Dzo buddy SKILL ("Phase 1 may call `legibility_scan`") is a thin UX wrapper;
the probe, scoring, and tracker conventions live in the repo (source of truth).

## Error handling

- usage.db missing/empty → index lane still runs (latent tier); recorder lane returns
  empty (mirror the existing `snapshot` graceful-default in `src/mcp_resources/tool_usage.rs`).
- file unparseable / unsupported language → skip + note; never fail the whole scan.
- tracker absent → create on first scan; concurrent scans → last-writer-wins on `params`
  (idempotent, acceptable).
- `RecoverableError` for input issues (bad project) → `isError:false`; `anyhow::bail!`
  only for genuine failures.
- **30-day retention is load-bearing on semantics:** a candidate truncated 14× two months
  ago shows 0 *current* friction → it correctly demotes tier 1 → tier 2. Friction that
  stopped is no longer biting. Documented to avoid surprise.

## Testing

- **Reconcile sandwich** (analog of the three-query cache-invalidation test): scan
  (candidate `open`) → shrink the body under budget → re-scan → assert `status:closed`
  + `after` delta. The stale→fresh proof for the reconcile path.
- **`overflowed` regression test:** `classify_content_result` returns `true` for the
  real `{output_id,…}` envelope and `false` for a normal result — guards the exact
  wrong-key defect.
- **F-1 regression test:** a `tool_calls` row with a different `project_root` is excluded
  from the scan — encodes the mirela-phantom lesson in code.
- **Gate test (tool-class drop):** recorder friction on a target with *no* structural
  defect → NOT a candidate. ⚠️ Per the project's "fallback gated on exact-match miss"
  lesson, the fixture must genuinely lack a structural defect, or it passes for the wrong
  reason. Assert on a path-specific marker.
- **`err_family` normalization:** table-test `error_msg → tag`.
- **Index lane:** fixture with an over-budget function → listed in tier 2; a small
  function → not listed.
- Env-resolved config (`LIBRARIAN_DB`, `LIBRARIAN_WORKSPACE`, …) → `EnvGuard` +
  `#[serial_test::serial]` per `docs/conventions/test-env-isolation.md`.

## Out of scope (YAGNI)

- Grep-roulette detection in the recorder lane (multi-alternation pattern clustering) —
  deferred to a later iteration; harder to attribute to a single target.
- CI-gating ("no new tier-1 defects on this PR") — the dry-run JSON makes it possible
  later, but not built now.
- Cross-project rollups (one tracker spanning codescout + mirela + southpole) — each repo
  keeps its own `usage.db` and its own backlog.
- Prompt-surface review: adding a new `librarian` action requires the standard
  prompt-surface review (server_instructions / onboarding / builders) — tracked as a
  plan task, not a design decision.

## References

- `docs/trackers/dzo-legibility-session-log.md` — F-1 (usage.db contamination), W-1.
- `docs/trackers/reconnaissance-patterns.md` — R-29 (verify flight-recorder targets exist in-repo).
- `docs/architecture/augmented-artifacts.md` — body/params/render_template, the `merge=false` foot-gun.
- `docs/superpowers/specs/2026-05-17-pika-observability-design.md` — `pika_observations` schema + "write on ask".
- `src/usage/mod.rs:97` `classify_content_result`; `src/usage/db.rs` `write_record`;
  `src/tools/core/types.rs:568` overflow envelope.
