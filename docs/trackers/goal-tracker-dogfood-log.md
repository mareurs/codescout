---
title: Goal-Tracker Dogfood Log
date: 2026-05-17
parent: docs/trackers/goal-tracker-cross-pollination.md
purpose: Tier-3 eval fixture set — observations from real use of the goal-tracker on its own implementation work.
---

# Goal-Tracker Dogfood Log

Real frictions observed while using the goal-tracker (`d2cd00fc837e53f2`)
to track its own cross-pollination work. Each entry is a Tier-3 eval
data point the spec didn't anticipate.

Each friction carries: when observed, what happened vs expected, root
cause hypothesis, fix pointer (issue # in audit, plan task, or new spec).

---

## DF-1 — gather returns empty `context: {}` for goal-trackers

**Observed:** 2026-05-17, on first `artifact_refresh(action=gather, id=L1)`.

**Status:** VERIFIED FIXED 2026-05-17.
- **Structural fix** in commit `c968391a` (T-3): `gather_goal_children` + structural detection in `refresh.rs::call` + 6 tests.
- **Empirical verification** after MCP reload: `context.deterministic_child_statuses` populates with 3 entries (C-1 active, C-2 done, C-3 in-progress, all `basis: deterministic`). `hints: ["3 items gathered from deterministic_child_statuses"]`. See W-5 in i1-session-friction.md.

**Related friction surfaced during verification:** F-9 (existing trackers retain stale prompts after `archetype_goal()` edits without explicit re-augmentation). The L1 still serves pre-T-4 prompt; pipeline still injects new context key. Workaround: re-augment L1 manually post-Phase-1.

**Expected (per prompt rule 1):** Each child's `.augmentation.params`
available to the synthesizer so it can normalize child status into the
goal's enum.

**Got:** Gather response had `"context": {}`. Only the goal's own params
+ body returned. No children fetched.

**Root cause:** The augmentation pipeline today has no notion of
"named dependencies" — the gather list (`gather_from`) supports
git_log, file, grep, artifacts (filter-based), observations, config_value,
but **not** "fetch these specific artifact IDs". The LLM is implicitly
expected to make `artifact(action="get")` calls mid-synthesis.

**Impact:** Every goal-tracker refresh is N+1 round trips (1 gather +
N child gets). For N=3 children that's 4 sequential calls just to
read state. The Stop hook does the same N+1 — find + get per goal.

**Fix:** This is the empirical validation of Yak's S4 / I1 plan
Task 3. Variant (b) `gather_goal_children` integration **must** be
the primary fix — it fetches all children in one batch and surfaces
them in the gather context. Resolves both this friction and the
deterministic-child-status feature.

**Pointer:** Plan task T-3 in `docs/superpowers/plans/2026-05-17-i1-refactor.md`.
Audit issue #37 (Yak S4) already maps here.

---

## DF-2 — `children[].status` in goal params can silently drift

**Observed:** At goal creation, I hand-set `children[].status` to
match what I knew about each child (C-1=active, C-2=done, C-3=in-progress).
Nothing validated these against the children's actual params.

**Expected:** Either (a) Rust derives `children[].status` at create
time from the linked artifacts, or (b) the create operation refuses
status values that don't match the child's actual state.

**Got:** Whatever the creator types in the params is what's stored.
Drift starts immediately if a child changes status between two
refreshes of the parent.

**Root cause:** `children[].status` is doubly-stored — once on each
child artifact, once mirrored in the parent's params. Mirror gets
stale between refreshes. The reconciliation prompt rule 1 exists
exactly to close this drift, but **before the first refresh**, the
mirror can already be wrong.

**Impact:** First-refresh-after-creation always has to reconcile
even if "nothing has changed". User confusion if they read the
parent's params verbatim and assume they reflect reality.

**Fix candidates:**

1. Don't store `children[].status` in the parent at all. Derive on
   read. Cost: every render fetches N children. Today's render
   template depends on the stored value.
2. Auto-derive `children[].status` at create time by fetching each
   linked artifact (cost: N fetches at create, then static).
3. Document the staleness; require a refresh after create to
   reconcile.

Tied to D5's `refresh_meta` work — `refresh_meta.children_status_delta`
becomes the canonical source for "what changed in this refresh."

**Pointer:** Discuss when T-3 / T-9 land. New audit issue: log here
for now, may promote to C-1 if it persists.

---

## DF-3 — `gather_from: git_log` mentioned in prompt but criterion can't drive it

**Observed:** First refresh. Prompt rule 3 says "evidence_commits:
commits added since last refresh that touched goal paths" but the
criterion is `"I1 refactor landed + Hamsa must-fixes shipped + ..."` —
unparseable into paths.

**Expected:** Either the criterion includes paths or the prompt
instructs the LLM how to map criterion → paths.

**Got:** LLM has to invent paths. Likely outcome: empty
`evidence_commits` list every refresh, defeating the purpose.

**Pointer:** Audit issue #1 (H-8) already captures this — same
class. Resolves alongside H-8 fix.

---

## DF-4 — Acceptance signals with non-freeform `kind` are accepted at creation but not enforced

**Observed:** Created acceptance signals with `kind: audit_issues_open_count`,
`kind: reflective_decided`, `kind: task_list_complete`. These are D4's
structured kinds, not yet implemented.

**Expected:** Either rejection (kind unknown) or accept-as-freeform
(D4's documented backward compat).

**Got:** Stored verbatim — `kind` field accepted because schema is
permissive (`additionalProperties` not constrained). But: the Rust
kernel doesn't evaluate them; the gather context doesn't enrich them;
the prompt doesn't know to read `evidence_child_id`.

**Impact:** Signal #2 (`reflective_decided` on C-2) has `met: true`
because I set it that way at creation. If C-2's underlying status
changed, no automation would catch the divergence.

**Root cause:** D4 isn't shipped yet (T-6). Pre-populating with
forward-compatible kinds was a bet; today the pre-population is
inert metadata.

**Fix:** T-6 implementation closes this. Until then, treat
non-freeform signals as freeform.

**Pointer:** Plan task T-6.

---

## DF-5 — No "you have unreconciled children" hint from gather

**Observed:** `"hints": []` in the gather response.

**Expected:** Some hint like "3 children linked; current reconciled
statuses may be stale — refresh recommended" or "0 children fetched
this gather; LLM must make N artifact_get calls".

**Got:** Nothing. The LLM gets the prompt + params + body + empty
context and is left to figure out the rest.

**Impact:** Sessions that pick up an existing goal-tracker mid-work
have no signal whether the parent's mirrored child statuses are
fresh. Stop hook surfaces last_refreshed_at now (T-14 shipped), but
gather doesn't echo the same.

**Fix:** When T-3 lands and gather populates
`deterministic_child_statuses`, add a hint like "fetched N children
deterministically; M required LLM interpretation; K orphaned".

**Pointer:** Adjacent to T-3.

---

## DF-6 — Prompt is the pre-Phase-1 70-line version (rule 1 still has the trench-coat table)

**Observed:** The gather response's `prompt` field is the **current**
augmentation prompt — i.e., the pre-I1 version with rule 1's 5-clause
table, rule 4a's `len(children) > 0` (not `>= 2` per D9), rule 6 NEVER
list, etc.

**Status:** PINNED AS EVAL BASELINE 2026-05-17 — original 70-line prompt
archived in this entry; live artifact has since been re-augmented
post-T-4 (see W-5 + F-9). The pre/post-fix prompt diff remains a
quantifiable Tier-3 eval signal independent of the artifact's current
state.

**Expected:** Obvious in hindsight — Phase 1 hasn't shipped. The
prompt is whatever's in `archetype_goal()` today.

**Got:** What we expected. Logging because this is the **eval
baseline** — if/when Phase 1 lands, re-running this same goal's
refresh should produce a measurably different prompt + better
behavior. The diff is the eval signal.

**Action:** Snapshot this gather response as a Tier-3 fixture. After
Phase 1 lands, snapshot again and diff. That diff is one of the few
quantifiable measures of whether I1 actually improves the refresh
pipeline.

---

## Snapshot — gather response 2026-05-17, pre-I1

(For diff comparison after Phase 1 lands.)

- `prompt`: 70 lines, rule 1 has 6 hard-coded per-archetype clauses.
- `params.children`: 3 entries with hand-set mirror statuses.
- `params.acceptance_signals`: 4 entries, 1 met (signal #2), 3 unmet.
- `context`: `{}` — empty.
- `hints`: `[]` — empty.
- `last_refreshed_at`: `null` — first refresh.
- Total response size: ~25 KB.

**Hypothesized post-I1 shape** (after T-3 lands):
- `context.deterministic_child_statuses`: array of 3 entries with
  derived `status` per child (`audit_issues` → "active", `reflective` →
  "done", `task_list` → "in-progress"), plus `basis: "deterministic"`.
- `hints`: includes "3/3 children resolved deterministically" or
  similar.
- Prompt: rule 1 collapses to "copy verbatim from context", ~10 lines
  vs current ~25 lines for rule 1.
