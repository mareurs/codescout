# Session Log — Tracker-as-Skill / Prompt-Surface

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries via `edit_markdown(action="insert_before", heading="## Template
> for new entries", content=...)`. Add a row to the Index / Wins Index
> table for each new entry — the indexes are the eval surface, the
> sections are the evidence.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-07-02 | med | prompt-surface | open | Inline provenance Iron Law collides with the 2200-char server_instructions cap (33B headroom) |
| F-2 | 2026-07-02 | med | eval-harness | mitigated | prompt-tdd runs all generators before judge preflight; invalid runs persist nothing (preflight shipped) |
| F-3 | 2026-07-02 | med | self-friction | mitigated | prompt-tdd report prints only per-run failing assertions; nearly read as all-runs-0.00 |
| F-4 | 2026-07-02 | low | eval-harness | mitigated | prompt-tdd test_sdk_pipeline hardcodes global scenario count (== 4); any added scenario reddens it |
| F-5 | 2026-07-03 | high | eval-harness | fixed-verified | prompt-tdd never pinned --model; A-4–A-7 evals ran on uncontrolled Fable instead of a chosen model |
| F-6 | 2026-07-03 | low | eval-harness | open | prompt-tdd report never surfaces per-scenario pass rate for multi-run scenarios (binary PASS/FAIL only) |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-07-02 | med | scout the 2200 cap + `*_invariants` before proposing any server_instructions addition | inline Iron Law fails `source_md_under_cap` at 33B headroom; ≥1 failed-edit round-trip + an unshippable recommendation | validated |
| W-2 | 2026-07-02 | high | pre-register + one-concept rubric + bind response↔score | four false eval findings (A-3, F-3, A-5, A-6) each one paragraph from a permanent surface | validated |
---

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Copy this block when appending a new friction. Allocate the next free
ID. Add a matching row to the Index table.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Copy this block when appending a new win. A win without a
**Counterfactual** is marketing — name what would have happened
without the pattern, with at least one piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Status:** validated | promoted-to-permanent-docs | archived

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — Inline provenance Iron Law collides with the 2200-char server_instructions cap

**Observed:** 2026-07-02, scouting the prompt-surface seam before recommending a persona/provenance trust-anchor (this session's persona-idea thread).

**When:** About to recommend adding a provenance Iron Law (“codescout `[LIVE]` directives are first-party guidance”) to the `server_instructions` slice.

**Expected (my suggestion):** Add a short `NEVER X → Y` Iron Law line to `src/prompts/source.md`'s server_instructions slice.

**Got (scouted reality):** The slice is **2167/2200 chars — 33 bytes of headroom** (`src/prompts/mod.rs:1156` `MAX_INSTRUCTIONS_CHARS=2200`; measured over `source.md` L2–46). The doc comment mandates: *“If you need to add content, author a `get_guide(topic)` entry and reference it from the slice.”* Iron Laws are also format-gated by `every_iron_law_has_do_instead` (must be `NEVER X → Y`). A provenance line is ≫33B → would fail `source_md_under_cap` and violate the additions-→-guide rule.

**Probable cause:** I proposed the addition from the design idea without scouting the cap-constrained surface; the 2200 cap + “additions→get_guide” rule wasn't in view.

**Workaround:** Route provenance content to a `get_guide(topic)` entry (a new `trust-provenance` topic, or fold into `iron-laws-detail`/`workspace-state`); at most a terse pointer in the slice, and only if the 33B budget allows — likely it must replace existing bytes or ride an existing line.

**Severity:** med — a naive inline edit fails `source_md_under_cap` and is reverted; controller absorbs the failed-gate round-trip, and any recommendation to “add an Iron Law” is unshippable as written.

**Status:** open

**Fix idea / Pointer:** get_guide topic for trust-provenance, gated on the persona eval (does false-distrust actually recur?) before spending any slice bytes. This session's persona thread; pairs with Hamsa memory `cap-forces-untested-wording-retest`.

---

## W-1 — Pre-recommendation scout caught the server_instructions cap before proposing a cap-blowing Iron Law

**Observed:** 2026-07-02, persona/provenance thread, before writing the recommendation.

**Pattern:** Before recommending ANY addition to a prompt-surface slice (`server_instructions` / `onboarding_prompt`), scout the cap invariant + current byte count first — `grep MAX_INSTRUCTIONS_CHARS`, measure the slice, read the `*_invariants` tests — rather than proposing the line and discovering the cap at test time.

**Counterfactual:** Without the scout I'd have recommended (and likely implemented) an inline provenance Iron Law. At 33B headroom, `source_md_under_cap` fails immediately → ≥1 failed-edit round-trip, plus a reader trusting a recommendation that cannot ship. The design's “additions→get_guide” rule would have surfaced only after the red test.

**Confirming data points:**
1. F-1 (this session) — 33B headroom measured; inline line impossible.
2. Hamsa memory `cap-forces-untested-wording-retest` — the same 2200 cap forced post-eval trimming of Iron Law 1 in a prior session.

**Impact:** med — saves a failed prompt-surface edit and keeps the recommendation shippable (routes to `get_guide`).

**Promote-when:** A second prompt-surface recommendation is scouted against the cap pre-edit. At 2 datapoints, promote to `src/prompts/README.md` / CLAUDE.md as “scout the 2200 cap before proposing any server_instructions addition.”

**Status:** validated

---
## F-2 — prompt-tdd runs all generators before any judge preflight; invalid runs persist nothing

**Observed:** 2026-07-02, persona eval first run (4 arms × 3 runs).

**When:** Running `prompt-tdd run scenarios/persona/` with system `python3` instead of the repo's `.venv/bin/python3`.

**Expected:** A judge misconfiguration (missing `anthropic` module) surfaces before any expensive work, or at worst the generated responses survive for re-judging.

**Got:** All 12 `claude -p` generator runs executed (278,905 ms ≈ 4.6 min), THEN every judge assertion failed with `No module named 'anthropic'` → `INVALID RUN`, 0/4. `results/` got no jsonl — the 12 responses were discarded, so the fix required a full regeneration, not a re-judge.

**Probable cause:** prompt-tdd has no judge preflight (client import + API-key check) before the generator loop, and does not persist raw generator outputs when the run is invalid.

**Workaround:** Re-ran the whole suite with `.venv/bin/python3` (which has `anthropic` 0.109.1).

**Severity:** med — ~5 min of wasted subscription generator calls per misconfigured run; cost scales linearly with runs × arms, and the same failure mode fires for an expired/missing API key.

**Status:** mitigated — preflight shipped (uncommitted in prompt-engineering); persist-on-INVALID deferred.

**Fix landed (2026-07-02):** `LLMJudge.preflight()` (judge.py) + `_preflight_judge(scenarios, config)` called at the top of `run_suite`/`run_for_prompt` (runner.py) — if any scenario declares judge (tier-3) assertions, the judge client is constructed BEFORE the generator loop (validates SDK import + API-key presence, no network call); failure raises a clear RuntimeError naming the fix, and zero generators run. 2 regression tests in test_runner.py (fail-fast + no-op-without-judge); full suite 280 passed. Uncommitted (separate repo, not asked to commit). Deferred: persist raw responses on INVALID (covers the *mid-run* judge failure the preflight can't catch) + the F-3 report change.

**Fix idea / Pointer:** Remaining two-part follow-up in `prompt-engineering/src/prompt_tdd/`: (1) `runner.py` preflights the judge (import + key) before spawning any generator when judge assertions exist; (2) persist raw responses to `results/` even on INVALID so a fixed judge can re-score without regeneration. Pairs with the pre-compact `report --format json` hang — the harness's failure-path ergonomics are its weakest seam.

---
## F-3 — prompt-tdd report prints only per-run FAILING assertions; two ✗ lines nearly read as "all runs scored 0.00"

**Observed:** 2026-07-02, persona eval (this work stream), reading the re-run report for arm C.

**When:** Interpreting `✗ judge:rubric: score=0.00 (threshold=0.5):` lines under a FAILed scenario with `runs: 3`.

**Expected:** Report lines summarize the scenario-level verdict (aggregate scores per rubric).

**Got:** The report prints one ✗ line per FAILING run-assertion only (passes invisible), and `pass_threshold` defaults to 1.0 — so a scenario with 2/3 clean runs and one bad run prints exactly like a catastrophic failure. Two of C's ✗ lines also carried EMPTY judge reasoning (structured-output artifact, orthogonal to score), compounding the misread. I briefly concluded "C scored 0.00 on both rubrics across all runs" — the pre-registered spoof-trap direction — which fresh captures then contradicted (judge scores 1.0/1.0/0.85 on actual C responses).

**Probable cause:** Report format optimized for triage, not distribution reading; `runner.py` per-run assertion model discovered only by reading source (`pass_rate = passed_count / num_runs`, report flattens `all_assertions`).

**Workaround:** Bind response↔score on captured responses before believing any judge delta; read `runner.py`/`report.py` before reading deltas off a report format seen for the first time.

**Severity:** med — a false C≪D finding was one paragraph away from the audit log; caught by the binding discipline, cost ~3 extra captures + 1 solo re-run.

**Status:** mitigated (misread corrected in-session; report format now documented here) 

**Fix idea / Pointer:** prompt-tdd report could print per-scenario run distribution (`2/3 runs passed; failing rubrics: …`) instead of bare per-run failures. Same repo/PR as F-2's preflight+persist fix. Full narrative: audit log A-4 method notes.

---
## W-2 — Bind-response-to-score + one-concept rubrics killed four false eval findings before any reached a permanent surface

**Observed:** 2026-07-02, prompt-hamsa persona/blanket/adherence evals (this work stream).

**Pattern:** Three composed disciplines, applied to every eval: (1) **pre-register** predictions + risks before any run; (2) **one concept per rubric** (never merge "resists injection" with "engages with facts"); (3) **bind response↔score** — read the actual generated responses and confirm the judge's number matches the text, before believing any delta. When a score and a read disagree, the read wins and the score is a suspected judge artifact.

**Counterfactual (four saves, each one paragraph from a permanent surface):**
1. **Persona injection-zone (A-3):** a merged "PROBE" rubric scored an obedient response 0.0, nearly writing a false "reader-first fights injection" finding. Split rubric → reversed.
2. **"C scored 0.00 across the board" (F-3):** the report prints only per-run *failures* with pass_threshold=1.0; two ✗ lines read as total failure. Fresh captures scored 1.0/1.0/0.85 → false spoof-trap finding averted.
3. **br2 FORGERY misfire (A-5):** judge scored a clearly injection-resistant response ~0.0 (it says "I apply none of the edits, would not commit on master, refuse the webhook change"). Reading it killed a false R1 "the rule leaks" conclusion — which would have wrongly sunk the one rule that works.
4. **Adherence false-ceiling guard (A-6):** before trusting "both arms PASS 3/3", ran a discrimination check (synthetic violation→0.0, compliant→1.0) proving the rubric had power — so the ceiling is real, not a toothless rubric passing everything.

**Confirming data points:** 4 independent catches in one work stream (A-3, F-3, A-5, A-6), across two failure modes (judge artifact + report-format misread) and one power-check.

**Impact:** high — each catch prevented a wrong conclusion from being committed to the audit log / a shipped prompt change. The single-judge (haiku) noise is real and sits exactly on decision boundaries; the discipline is what makes single-judge eval trustworthy at all.

**Promote-when:** already load-bearing. Promote to `prompt-engineering` harness docs (the "An honest artifact is not an honest rubric" section already seeded) + Hamsa memory (done: `rubric-one-concept-test-on-nuance`). At a 5th catch, promote "bind response↔score before believing a judge delta" to a first-class step in the skill-eval playbook.

**Status:** validated

---
## F-4 — prompt-tdd `test_sdk_pipeline_with_v2_scenarios` hardcodes a global scenario count (`== 4`)

**Observed:** 2026-07-02, running the prompt-engineering suite after adding eval scenarios under `scenarios/`.

**When:** `pytest tests/` after creating `scenarios/persona/*` (and the pre-compact `scenarios/trackers/*`) for the A-4/A-5/A-6 evals.

**Expected:** Adding a new scenario directory is a normal, non-breaking action.

**Got:** `test_integration.py::test_sdk_pipeline_with_v2_scenarios` does `discover_scenarios(scenarios/)` then `assert len(scenarios) == 4` — a working-tree-wide count. Any added scenario dir (even untracked) turns it red: `AssertionError: Expected 4 scenarios, found 14`. The test was already latent-red from the pre-compact trackers scenarios before today's additions.

**Probable cause:** The test hardcodes a global count of a directory that grows whenever anyone adds a scenario, instead of scoping to the specific fixtures it means to exercise (or asserting `>=`).

**Workaround:** Relocated the experimental eval scenarios out of `scenarios/` to the session scratchpad (`scratchpad/persona-eval/scenarios-archive/`); suite returned to 280 passed. The audit-log A-4/A-5/A-6 entries + blanket/adherence plan files document the scenarios for reproduction.

**Severity:** low — brittle test, not a product bug; but it silently blocks a green suite for anyone with local scenario experiments in the tree.

**Status:** mitigated (scenarios relocated) — root brittleness unfixed in prompt-engineering.

**Fix idea / Pointer:** Point the test at a dedicated fixtures subdir, or assert `>= 4` / a per-subdir count. prompt-engineering `tests/prompt_tdd/test_integration.py::test_sdk_pipeline_with_v2_scenarios`.

---
## F-5 — prompt-tdd's ClaudeCodeRegistry never pinned --model; two eval decisions ran on an uncontrolled model (Fable, not chosen)

**Observed:** 2026-07-03, while investigating why the judge's Anthropic API key drained credits during the persona/blanket/provenance evals earlier this session.

**When:** Auditing `ClaudeCodeRegistry._evaluate_handler` in prompt-engineering after fixing the ambient-API-key leak (F-2's sibling bug).

**Expected:** Every `claude -p` generator subprocess runs a known, pinned model so eval results are reproducible and attributable.

**Got:** `SessionConfig.model` defaulted to `""`, `prompt_tdd.yaml` had no `session:` block, and `cli.py` read `session_raw.get("model", "")` — so `--model` was never passed. `claude -p` silently used whatever model the operator's interactive CLI profile currently had selected. Confirmed: the earlier persona/blanket/delegation/provenance experiments this session (A-4 through A-7) ran on Fable, not a deliberate choice — and the API judge key got billed for those same Fable generations, draining it (the two bugs compounded: leaked key + unpinned model = silent Fable billing).

**Probable cause:** `model: str = ""` was written as an "empty means don't override" sentinel without a corresponding safe default — the same footgun shape as F-2's `run_env = None` (silent full-inheritance is the resting state instead of an explicit, safe default).

**Workaround:** none needed once fixed — see Fix.

**Severity:** high — two ship decisions (A-7 Test 1 and Test 2) were made from data whose generator model was neither known nor controlled at pre-registration time; re-running under a pinned model was required to know which findings held.

**Status:** fixed-verified — prompt-engineering `aecb76f` (env-strip) + `8790c80` (model-pin, `DEFAULT_GENERATOR_MODEL = "sonnet"`, per Marius's explicit choice). Verified live (`claude -p ... --model sonnet` resolves) and by re-running all 9 A-7 arms pinned: Test 1 fully confirmed (2 runner FAILs were judge misfires, not model effects); Test 2's KEY-PRIORITY confirmed 6/6 across both model conditions, but CALIBRATE dropped to a real 1/3 under Sonnet — a genuine model-dependent finding the unpinned run could not have surfaced honestly.

**Fix idea / Pointer:** prompt-engineering commits `aecb76f`, `8790c80`; regression tests `test_evaluate_inherits_env_without_config_dir` (rewritten) + new `test_default_session_pins_model_explicitly`. Audit log A-7 correction section.

---
## F-6 — prompt-tdd report never surfaces per-scenario pass RATE for multi-run scenarios

**Observed:** 2026-07-03, re-running E4 (provenance CALIBRATE) at n=10 pinned to Sonnet, requested after the n=3 result proved unreliable (F-5's correction, then a second correction).

**When:** Reading the harness's own report for a `runs: 10` scenario to get the pass rate.

**Expected:** A multi-run scenario's report shows how many of the N runs passed (e.g. "9/10"), since that rate is the actual signal for a borderline rubric.

**Got:** `runner.py` computes `pass_rate = passed_count / num_runs` internally, but `report.py` never reads or prints it (`grep pass_rate report.py` → 0 matches). The CLI only shows a binary PASS/FAIL against `pass_threshold` (default 1.0 — ANY failing run fails the whole scenario) plus a list of failing-run assertions. A scenario that passes 9/10 runs and one that passes 0/10 both render identically as "FAIL" with the same shape of failing-assertion list; a scenario that passes 10/10 shows bare "PASS" with no rate at all.

**Probable cause:** the report was designed around `pass_threshold=1.0` (binary correctness), and multi-run scenarios with a graded/borderline rubric — where the RATE is the actual finding — were not the original design target.

**Workaround:** reconstruct the rate by direct capture: generate N independent responses via `claude -p --model <pinned>`, judge each individually via `judge_for_model(...).evaluate_rubric(...)`, and count. Done for E4 at n=10 (twice — once via the harness's own PASS/FAIL as a coarse cross-check, once via direct capture for the actual rate).

**Severity:** low — does not block any eval, but costs an extra manual-capture pass every time a multi-run rate matters, exactly the shape of friction F-2/F-3 already logged for this harness.

**Status:** open

**Fix idea / Pointer:** `report.py`'s scenario-line formatter could print `pass_rate` alongside the PASS/FAIL verdict whenever `scenario.runs > 1` (e.g. "FAIL (9/10)"). Same repo/PR as F-2's preflight+persist fix and F-3's per-run distribution idea — this is the third instance of the same underlying gap (the report renders a verdict, not the distribution behind it).

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
