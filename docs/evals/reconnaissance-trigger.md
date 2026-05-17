# Reconnaissance Skill — Trigger Eval

**Purpose:** Score the SKILL.md frontmatter `description:` string against representative
prompts to verify the trigger fires when intended and stays silent when not.

**Status:** Bootstrap. Cases pinned + baseline predicted by inspection. **Not yet run
empirically against a model — n=0 graded runs.**

**Owner:** Hamsa (auditor) + Snow Lion (architectural verdict on cases).

---

## How to run

1. Show the model the SKILL.md description under test (and competing skill descriptions
   if available — `subagent-driven-development`, `writing-plans`,
   `verification-before-completion`, `executing-plans`).
2. For each Case below, present the **Prompt** verbatim and ask:
   *"Would you invoke the `reconnaissance` skill here? Answer YES or NO, then one
   sentence why."*
3. Score against **Expected**: PASS = match, FAIL = mismatch.
4. **AMBIGUOUS** (hedging, "maybe", "depends") counts as FAIL — trigger string must be
   unambiguous to a stranger.
5. Tally. Ship threshold: **≥6/7**. Below threshold = rewrite the description.

## Rubric

| Verdict | Criterion |
|---|---|
| PASS | Model's invoke decision matches Expected verbatim |
| FAIL | Mismatch, or model hedges, or model picks reconnaissance for the wrong reason |
| AMBIGUOUS | Model defers to user; counted as FAIL |

A *right-answer-wrong-reason* PASS is downgraded to FAIL — the trigger string must
attract the right scenarios, not coincidentally happen to match.

---

## Cases

### Case 1 — Subagent-eligible plan task

**Prompt:** *"Implement T-6 — split `progress_log` into `refresh_meta` (Rust-owned) and
`progress_log[].note` (LLM-owned)."*

**Expected:** TRIGGER

**Reasoning:** Plan-driven task, structural schema change to params, likely subagent
dispatch. Precedents: W-1 (scout-before-dispatch saved T-2 from F-5 recurrence), F-8
(plan code can be fictional — verify before dispatching).

---

### Case 2 — Struct shape change without prior read

**Prompt:** *"Update `child_status_pure` to take `&ChildContext` instead of `&Value`."*

**Expected:** TRIGGER

**Reasoning:** Signature change against a struct we may not have read. F-8 precedent:
T-3 plan invented `cat.get(id).augmentation.archetype` (3 compile errors — `ArtifactRow`
has no `augmentation` field; correct path is `augmentation::get(&cat, ...)`;
`AugmentationRow` has no `archetype` field). Reconnaissance must scout the type first.

---

### Case 3 — Tool/plan disagreement

**Prompt:** *"I just called `artifact_refresh(gather)` and the context came back empty —
that's not what the plan predicted."*

**Expected:** TRIGGER

**Reasoning:** Externalize as F-N. Precedents: DF-1 (empty gather context, structurally
fixed in c968391a), F-9 (tracker drift — re-augmenting L1 needed to pick up T-4
changes), W-4 (release-binary footgun: dev build invisible to MCP). Plan-vs-reality
gap is the canonical reconnaissance trigger.

---

### Case 4 — Read-only Q&A

**Prompt:** *"What does `OutputGuard::cap_items` do?"*

**Expected:** SKIP

**Reasoning:** No edit, no dispatch, no plan dependency. Reading source via `symbols`
is the answer; reconnaissance would be ceremony layered on a one-step lookup.

---

### Case 5 — Trivial mechanical edit

**Prompt:** *"Bump version to 0.5.2 in `Cargo.toml`."*

**Expected:** SKIP

**Reasoning:** No shape contact, no plan, no dispatch, no callers. Pure mechanical.
Invoking reconnaissance here trains the model to invoke it on everything → noise.

---

### Case 6 — Refactor with multi-caller impact (BOUNDARY)

**Prompt:** *"Refactor `gather_all` for readability."*

**Expected:** TRIGGER

**Reasoning:** Boundary case. Refactor touches caller invariants. Snow Lion Heuristic 4:
"If adding a feature requires modifying more than three modules, suspect the boundaries
are wrong" — same shape applies to refactor. Without scouting callers via `call_graph`,
the refactor may distort flow. We declare TRIGGER because "refactor" + multi-caller
implies shape contact; if model declines to TRIGGER here it's a genuine architectural
choice we'd debate, not a bug.

**Note:** This case exists specifically to pressure-test the trigger's precision. If
the rewrite passes 6 and 7 simultaneously, the trigger is anchored well.

---

### Case 7 — Doc-only edit on existing symbol

**Prompt:** *"Add a doc comment to `gather_goal_children` explaining the deterministic-
status path."*

**Expected:** SKIP

**Reasoning:** Doc-only, no behavioral change, no shape change. The trigger string must
NOT fire on "edits a function" alone — only on edits that change shape or contracts.
If reconnaissance fires here, the trigger is over-broad.

---

### Case 8 — Refactor with named shape contact (BOUNDARY → TRIGGER)

**Prompt:** *"Refactor `gather_all` to take a `&Catalog` parameter explicitly instead of pulling from `ctx`."*

**Expected:** TRIGGER

**Reasoning:** Refactor that NAMES a signature change in the prompt itself. Distinguishes from Case 6 (refactor without named shape) and Case 9 (refactor purely internal). If the description correctly fires here while skipping Case 9, the refactor-shape distinction is anchored.

---

### Case 9 — Refactor purely internal (BOUNDARY → SKIP)

**Prompt:** *"Refactor `child_status_pure` to use a single `match` expression instead of nested `if let` blocks."*

**Expected:** SKIP

**Reasoning:** Internal restructure, no signature change, no caller impact, no API shape contact. Should NOT trigger reconnaissance. Together with Case 8, brackets the refactor-precision question.

---

### Case 10 — Compile error during routine work (SKIP, competing skill)

**Prompt:** *"`cargo build` failed with `cannot find function 'frobnicate' in this scope` — fix it."*

**Expected:** SKIP

**Reasoning:** This is `systematic-debugging` territory. A compile error from a missing identifier is bug-fix work, not plan-vs-reality scout. Reconnaissance phase 2 (compare expected to got) is about plan drift, not toolchain output.

---

### Case 11 — About to claim work complete (SKIP, competing skill)

**Prompt:** *"All 14 I1 tasks landed. Ready to cherry-pick to master."*

**Expected:** SKIP

**Reasoning:** `verification-before-completion` territory. Reconnaissance is at the seam (before an edit / dispatch / decision), not at the completion gate. Distinct timing.

---

### Case 12 — Plan task with multi-file integration (TRIGGER)

**Prompt:** *"Wire `gather_goal_children` into `refresh.rs::call` so the deterministic statuses flow through to the context HashMap."*

**Expected:** TRIGGER

**Reasoning:** Plan task, multi-file integration, struct/API shape contact (`refresh.rs::call` signature + HashMap key naming + return shape). F-8 precedent: plan code that names multiple symbols can be fictional — scout first.

---

### Case 13 — Test addition for already-scouted symbol (SKIP)

**Prompt:** *"Add a unit test for the `audit_issues` branch of `child_status_pure`."*

**Expected:** SKIP

**Reasoning:** Test addition for code already in scope (existing handler, existing test module). No new shape contact, no plan drift surface. Reconnaissance overkill if invoked.

---

### Case 14 — Plan-status query (SKIP, pressure-tests "multi-task work")

**Prompt:** *"I just finished 5 tasks in the I1 refactor. What's next?"*

**Expected:** SKIP

**Reasoning:** Status query about plan progress. The trigger string's *"at the start of multi-task work"* phrase could over-fire here ("I'm in a multi-task session, planning what's next"). This case discriminates whether the model correctly bounds *"start of multi-task work"* to "starting a new task" rather than "operating within a multi-task session."

---

### Case 15 — Architectural decision needing shape scout (BOUNDARY → TRIGGER)

**Prompt:** *"Should `metric_baseline` move into the Rust kernel like the other archetypes, or stay LLM-evaluated?"*

**Expected:** TRIGGER

**Reasoning:** Architectural decision where the right answer depends on understanding current shape: how is `metric_baseline` currently evaluated? What are its inputs? What's the kernel's contract? Scout BEFORE deciding. Competing skill `brainstorming` could also fire, but reconnaissance is the right primary because the decision is shape-bounded, not preference-bounded.

---

## Baseline Score — Predicted (inspection, not measurement)

**Draft under test:**

> *"Use before subagent dispatch, before structural edits that depend on struct/API
> shapes, after ANY surprise from the plan or expectations, and at the start of
> multi-task work. Externalizes drift findings to a session-log tracker (F-N/W-N/V-N
> entries) so the discipline compounds across sessions."*

**Predicted scoring (Hamsa-as-judge stand-in):**

| Case | Expected | Predicted | Verdict | Why |
|------|----------|-----------|---------|-----|
| 1 | TRIGGER | TRIGGER | PASS | "subagent dispatch" matches cleanly |
| 2 | TRIGGER | TRIGGER | PASS | "structural edits that depend on struct/API shapes" matches |
| 3 | TRIGGER | TRIGGER | PASS | "ANY surprise from the plan" matches (but over-broad reason) |
| 4 | SKIP     | TRIGGER | **FAIL** | "ANY surprise from… expectations" can read as "this answer surprised me" |
| 5 | SKIP     | TRIGGER | **FAIL** | "at the start of multi-task work" — session has 8+ pending tasks → over-fires |
| 6 | TRIGGER | TRIGGER | PASS | "structural edits" matches |
| 7 | SKIP     | TRIGGER | **FAIL** | "structural edits" could read as "edits to a function" |

**Predicted: 4/7. Below ship threshold (6/7).**

**Confirms Hamsa diagnosis:** trigger over-fires on `ANY`, `multi-task work`, and the
unanchored "structural edits" qualifier. Three FAIL cases all map to the three cuts
Hamsa flagged. Rewrite earns its keep IF it scores ≥6/7 against the same set.

**Caveat (Hamsa Self-Trap 5):** This baseline is inspection-based. Predicting model
behavior without running the model is fiction. Treat 4/7 as a *hypothesis* until the
prompts are actually run with the description in context. Until then, every rewrite
verdict is unverified.

---

## Status

- [x] Rubric pinned
- [x] 7 cases drafted (3 TRIGGER, 3 SKIP, 1 BOUNDARY → TRIGGER)
- [x] Baseline predicted (4/7, inspection-only)
- [ ] **Baseline empirical run (n=0 — UNVERIFIED until done)**
- [ ] Score rewrite candidate v1
- [ ] Iterate description until ≥6/7
- [ ] Ship description into SKILL.md
- [ ] Optional: expand to 20-50 cases per Hamsa Heuristic 7

## Iteration log

_(Append one row per scoring run. First row should be the empirical baseline.)_

| Date | Description version | Cases passed | Notes |
|------|---------------------|--------------|-------|
| 2026-05-17 | v0 (current draft) | 4/7 predicted | Inspection only — not empirically run |
| 2026-05-17 | v0 (current draft) | **6/7 empirical** | Fresh `general-purpose` subagent, no project context. Hamsa's prediction wrong on Cases 4, 5, 7 (predicted FAIL → actual PASS). Only true FAIL is Case 6 (refactor — AMBIGUOUS counts as FAIL). At ship threshold. |
| 2026-05-17 | **v0.1 (shipped)** | 6/7 inherited | Path A: replaced `V-N` with `F-N/W-N` in the parenthetical taxonomy reference. Trigger phrases unchanged → baseline transfers without re-scoring. SKILL.md written to `claude-plugins/codescout-companion/skills/reconnaissance/SKILL.md`. |
| 2026-05-17 | v0.1 (15 cases) | **12/15 empirical** | Expanded eval 7 → 15 cases (added 8: refactor-with-shape-named, refactor-internal, compile-error, completion-claim, multi-file integration, test-for-scouted-symbol, plan-status query, architectural decision). 3 FAILs: Case 6 AMBIGUOUS (refactor unanchored — same as 7-case run), Case 14 AMBIGUOUS ("multi-task work" unanchored on plan-status query), Case 15 NO/expected-YES (subagent routed to brainstorming — case-design questionable; regrading to SKIP would land 13/15 = at threshold). Below 87% threshold; refactor + plan-context phrases now have TWO concretes — rewrite earned per two-concretes rule. |
| 2026-05-17 | **v0.2 (shipped)** | **13/15 empirical** | Cut *"at the start of multi-task work"* + *"ANY"* + replaced *"structural edits that depend on struct/API shapes"* with explicit *"struct, function signature, or API contract"*. Case 14 fixed (NO, was AMBIGUOUS). Cases 6 + 15 unchanged — now genuinely case-design issues, not description issues (Case 6 expected was always hedged; Case 15 routes to `brainstorming` defensibly). With those two regrades, score would be 15/15. At ship threshold without regrades. |
## Re-evaluation after baseline

**Inversion of Hamsa's diagnosis:** the trigger string is at the ship threshold without
any rewrite. Three of four predicted failures (Cases 4, 5, 7) did not materialize. The
model — when shown competing skill descriptions in realistic skill-selection context —
correctly bounded the vague phrases (`ANY surprise`, `multi-task work`, `structural
edits`) rather than over-firing on them.

**Implications:**

1. **The proposed cuts (`ANY`, `multi-task work` tail, qualifier-on-`structural`) are
   no longer load-bearing for the score.** They may still be defensible on prose-quality
   grounds (cleaner = better, shorter description weighs less in long-context skill
   selection), but they will not move 6/7 → 7/7 unless Case 6 is also addressed.

2. **Case 6 is the only genuine FAIL** and it pressure-tests refactor coverage. Two
   readings are open:
   - **Description fix:** add explicit refactor language (`"refactor that touches ≥3
     callers"` style) — but this may overfit to one case
   - **Case fix:** accept AMBIGUOUS as correct model behavior on underspecified refactor
     prompts, downgrade case-design rather than description

3. **Methodology lesson (record permanently):** inspection-based critique mispredicted
   75% of failures (3 of 4). The eval substrate is now mandatory before any future
   trigger-string rewrite. Hamsa Self-Trap 5 confirmed empirically in a 1-shot run.

**Decision point owed to next session:** ship v0 as-is (6/7 at threshold), or pursue
optional cleanups that may not move the score?
