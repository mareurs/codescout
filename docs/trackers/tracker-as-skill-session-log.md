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
| F-2 | 2026-07-02 | med | eval-harness | open | prompt-tdd runs all generators before judge preflight; invalid runs persist nothing |
| F-3 | 2026-07-02 | med | self-friction | mitigated | prompt-tdd report prints only per-run failing assertions; nearly read as all-runs-0.00 |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-07-02 | med | scout the 2200 cap + `*_invariants` before proposing any server_instructions addition | inline Iron Law fails `source_md_under_cap` at 33B headroom; ≥1 failed-edit round-trip + an unshippable recommendation | validated |
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

**Status:** open

**Fix idea / Pointer:** Two-part fix in `prompt-engineering/src/prompt_tdd/`: (1) `runner.py` preflights the judge (import + key) before spawning any generator when judge assertions exist; (2) persist raw responses to `results/` even on INVALID so a fixed judge can re-score without regeneration. Pairs with the pre-compact `report --format json` hang — the harness's failure-path ergonomics are its weakest seam.

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
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
