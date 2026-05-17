---
id: '0df5ebc95d284b8e'
kind: tracker
status: draft
title: Goal-Tracker Cross-Pollination Findings Audit
owners: []
tags:
- audit
- goal-tracker
- cross-pollination
- audit_issues
topic: null
time_scope: null
---

## Audit scope and methodology

Findings from the 2026-05-17 review session on the goal-tracker archetype
implementation and its boundary with `audit_doc_refs` (the tracker → code
sync sibling feature). Three reviewers contributed:

- **Hamsa** (prompt gatekeeper) — audited the augmentation prompt, the
  discovery paragraph in `server_instructions.md`, the Stop hook reason
  strings, and the `when_to_use` field. First pass + deep pass.
- **Lion** (architecture) — boundary placement, coupling, change-scenario
  validation, container-pattern critique. First pass + deep
  synth-mechanism-inversion pass.
- **Yak** (refactor mechanics) — stress-tested the I1 refactor proposal,
  found three seams blocking clean landing, sequenced the actual landing.

Audit closed 2026-05-17. Resolutions tracked in
[goal-tracker-amendment](../superpowers/specs/2026-05-17-goal-tracker-amendment.md)
(decisions D1–D11) and
[i1-refactor plan](../superpowers/plans/2026-05-17-i1-refactor.md) (tasks T1–T14).

## Per-issue detail

Each numbered issue carries: source reviewer, severity, status, and the
resolution pointer. Open issues are listed first; closed issues are kept
for audit trail.

### Open findings (need code/spec/style work)

- **#2 — H-9** *(open, med)*: `date: today` has no resolution mechanism.
  Prompt mentions `today` but no placeholder syntax; model guesses.
  Fix candidate: surface today's date as a templated gather param.

- **#3 — H-10** *(open, low)*: History append format unstructured.
  Body skeleton shows `_### YYYY-MM-DD — <event>_` but prompt doesn't repeat
  the format. Model may append inconsistently.

- **#4 — S-3** *(open, low)*: Stop hook active-branch surfaces only first
  unmet signal, no count. Reader cannot tell "1 of 4 left" from "1 of 1 left".

- **#6 — W-2** *(open, low)*: `when_to_use` field includes "survives across
  sessions" — decoration, doesn't discriminate goal from other trackers.

- **#7 — C-4** *(open, high)*: No eval set. Every finding above is an
  inspection, not a measurement. Tier 3 eval needs ANTHROPIC_API_KEY +
  10+ graded refresh scenarios. Separate effort.

- **#8 — A-2** *(open, high)*: No Rust-side contract test for the CLI
  envelope shape the bash Stop hook depends on. Fail-open semantics hide
  the regression if `artifact find --json` ever stops emitting `.count` or
  `.items[].id`. Need `cargo test` that pins the envelope.

- **#9 — A-4** *(open, low)*: Spec language "Container pattern" (capitalized)
  is mildly premature — names an abstraction before there are two
  concrete containers. Style fix only.

- **#10 — A-5** *(open, low)*: Spec headline "new goal types require zero
  new archetypes" overclaims — only true for new *compositions* of existing
  signal shapes. Novel signal shapes still need new child archetypes.
  Style fix in original spec.

- **#11 — Q4 / I9** *(open, high)*: Stop hook should be Rust (CLI subcommand
  `codescout goal stop-decide`), not bash + jq + Haiku. 7-branch decision
  tree, deterministic. Spec acknowledges "status-reader only, not
  progress-judge" — i.e. LLM has no job. Cost + latency win. Plan marks
  this out-of-scope for Phases 1-4; tracked here for follow-up.

### Closed by amendment + plan

- **#1 — H-8** *(fixed)*: `evidence_commits` since-last-refresh anchoring fully landed. T9 (commit `20b5ba1a`) added `refresh_meta.last_refresh_at` + `refresh_meta.commit_count_since_last` to the deterministic gather context; this session (commit `44632d3e`) updated rule 3 of the goal-tracker `prompt_template` to explicitly instruct the LLM to select `evidence_commits` from `context.git_log` restricted by `refresh_meta.last_refresh_at`, and `evidence_artifacts` from `refresh_meta.children_status_delta`. Regression test `goal_prompt_rule_3_anchors_evidence_fields_to_gather_context` asserts all three required anchors. L1 dogfood `d2cd00fc837e53f2` re-augmented with the new prompt.
- **#5 — S-4** *(fixed)*: Done-branch reason now carries gate evidence. The companion repo's `goal-stop-hook.sh` queries `codescout artifact-event list --kinds note` for the goal, filters for `payload.tag == "gate_check" AND payload.gate_passed == true`, takes the newest, and appends `payload.text` (e.g. "auto-close gate passed: 3/3 children done, 4/4 signals met") to the reason — fail-soft fallback to the legacy form if no event found. Matrix test extended 8 → 9 branches; live probe verified CLI shape matches the jq filter end-to-end against L1 dogfood event. Shipped companion-repo commit `ab0364c`.
- **#12 — H-1** *(fixed)*: `deployment_state` missing from prompt rule 1 → resolved by **D3** + plan **T1** (constants) + **T2** (predicate).
- **#13 — H-2** *(fixed)*: "Re-evaluate" verb contradicts "do not recompute" framing → resolved by **T4** prompt edit.
- **#14 — H-3** *(fixed)*: Children-free goal gate-locked forever → resolved by **D9** (`len(children) >= 2`) + **T4** prompt edit.
- **#15 — H-4** *(fixed)*: Empty `task_list` vacuously done → resolved by **D1** (`Pending`) + **T2** predicate.
- **#16 — H-5** *(fixed)*: No clause for archetype not in rule 1 table → resolved by **T2** default case → `Unknown`.
- **#17 — H-6** *(fixed)*: Scope-growth uncapped → resolved by **D10** (1/refresh) + **T11** validator.
- **#18 — H-7** *(fixed)*: Rule 6 mostly negations of positive rules → resolved by **D6** + **T10** (NEVER list collapses to Rust guarantees).
- **#19 — H-11** *(fixed)*: Same-model self-critique trap (4c mitigation lives in spec, not prompt) → resolved by **D11** + **T12** (`note` event with `gate_check` tag).
- **#20 — D-1** *(fixed)*: At-most-one-active-goal soft norm with no enforcement → resolved by **T4** discovery paragraph adds consequence sentence.
- **#21 — D-2** *(fixed)*: Discovery paragraph no "not for" guidance → resolved by **T4**.
- **#22 — D-3** *(fixed)*: Discovery omits nested goal as legal child → resolved by **T4**.
- **#23 — S-1** *(fixed)*: Stop hook reads stale state, no `last_refreshed_at` in reason → resolved by **T14** (shipped commit `0b75991` on 2026-05-17).
- **#24 — S-2** *(fixed)*: Unknown status indistinguishable from active → resolved by **T14** (same commit `0b75991`).
- **#25 — W-1** *(fixed)*: Gate forces children but `when_to_use` doesn't carve out → resolved by **D9** + **T4** `when_to_use` tightening.
- **#26 — C-1** *(fixed)*: Three surfaces / three multiplicity stances → resolved by **T4** unifying stance across prompt + discovery + hook output.
- **#27 — C-2** *(fixed)*: Per-archetype reconciliation is 5 prompts in a trench coat → resolved by **Phase 1** (T1–T5) moving rules to Rust kernel.
- **#28 — C-3** *(fixed)*: Audit step (verdict event) lives in spec, not prompt → resolved by **D11** + **T12**.
- **#29 — A-1** *(fixed)*: Per-archetype rules concentrated coupling → resolved by **Phase 1** (kernel extraction).
- **#30 — A-3** *(fixed)*: `verdict` event kind misuse (existing semantics conflict) → resolved by **D11** (use `note` with `tag: gate_check`).
- **#31 — F-A** *(fixed)*: `progress_log` should split into `refresh_meta` + `note` → resolved by **D5** + **T9**.
- **#32 — F-B** *(fixed)*: NEVER list is architecture upside-down → resolved by **D6** + **T10** (each NEVER becomes Rust type/function guarantee).
- **#33 — F-D** *(fixed)*: `audit_issues` archetype schema mismatch with audit-doc-refs runtime → resolved by **D7** + **T13** (widen archetype to fold optional fields).
- **#34 — Yak S1** *(fixed)*: Schema drift between archetype example and runtime → resolved by **T2** defensive parsing (every predicate handles missing/empty/wrong-type defensively).
- **#35 — Yak S2** *(fixed)*: `metric_baseline` cross-cutting context blocks pure-function signature → resolved by **D4** + **D8** + **T6** + **T7** (structured `acceptance_signals` + `child_status_in_context`).
- **#36 — Yak S3** *(fixed)*: Rule 2 is harder synth, I1 didn't address it → resolved by **D4** + **T6** (rule 2 also gets structured kinds, Rust-evaluable).
- **#37 — Yak S4** *(fixed)*: Pipeline has no "call Rust mid-prompt" path → resolved by **T3** adopting variant (b) (gather-time injection).
- **#38 — Yak S5** *(fixed)*: Test seam — idempotency property test → resolved by **T2** test `idempotent_pure_function`.

## History

### 2026-05-17 — Audit opened + closed in single session

All 38 findings logged. 27 closed by amendment doc (D1–D11) + plan doc
(T1–T14). 11 remain open: 4 medium (H-8, H-9, S-4, A-2) need targeted
follow-ups; 4 low (H-10, S-3, W-2, A-4, A-5) are cosmetic / can wait;
3 high (C-4 eval gate, A-2 contract test, Q4 Stop-hook-as-CLI) are
separate efforts tracked outside this audit.

### 2026-05-17 — H-8 closed via goal-tracker prompt edit

H-8 (`evidence_commits` anchoring undefined) flipped from
*(open, med, partial-mitigation)* to *(fixed)*. Rule 3 of the
goal-tracker `prompt_template` now explicitly tells the LLM to populate
the four progress_log fields from the Rust-supplied gather context:
`date` (today UTC), `note` (one-line summary), `evidence_commits`
(filtered subset of `context.git_log` after `refresh_meta.last_refresh_at`),
and `evidence_artifacts` (looked up via `refresh_meta.children_status_delta`).
New test `goal_prompt_rule_3_anchors_evidence_fields_to_gather_context`
carves out rule 3 and asserts the three required anchors plus a
regression guard against the old "note is the only LLM-owned field"
claim. L1 dogfood `d2cd00fc837e53f2` re-augmented with the new prompt;
the `ON CONFLICT DO UPDATE` clause in `augmentation::upsert` preserved
`created_at` / `last_refreshed_at` / `refresh_count` automatically.
Counts now: 28 fixed / 10 open.

### 2026-05-17 — S-4 closed via companion Stop hook gate_check surfacing

S-4 (`done reason has no gate evidence`) flipped from
*(open, med, partial-mitigation)* to *(fixed)*. The codescout side had
been emitting `note` events with `payload.tag == "gate_check"` carrying
`text` + structured `evidence` since T-12 + D11 (codescout commit
`1e76a43c`); the companion Stop hook just wasn't reading them. The
hook's `done)` branch now queries `codescout artifact-event list
--artifact-id <goal_id> --kinds note --limit 20 --json`, selects the
newest event with `payload.gate_passed == true`, and appends
`payload.text` to the reason. Fail-soft fallback: if no matching
event is found (legacy goals, status flipped by hand, or the
subcommand errors), the hook falls back to the legacy reason form so
no regression for goals that never crossed the auto-close gate.
Matrix test (`goal-stop-hook.matrix.test.sh`) extended from 8 to 9
branches: branch 3 (existing done) now asserts the gate text appears
in the reason; branch 9 (new) covers done + no gate event and asserts
the fallback works without leak (defensive substring-absent assertion).
Live probe against the real codescout binary + L1 dogfood's
gate_check event confirmed the CLI shape matches the jq filter
end-to-end (`auto-close gate passed: 3/3 children done, 4/4 signals
met`). Shipped companion-repo commit `ab0364c`. Counts now: 29 fixed
/ 9 open.### 2026-05-17 — First fix shipped

Hamsa S-1 + S-2 (issues #23 + #24) closed by commit `0b75991` in
`codescout-companion`. Stop hook now surfaces `last refreshed: <ts>`
in every reason branch + distinguishes malformed-status from active.
Matrix test extended 7 → 8 branches; all green.
