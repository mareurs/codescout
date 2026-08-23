---
id: '556cc34167321863'
kind: spec
status: draft
title: Hidden-information eval — does codescout help or hinder?
tags:
- eval
- prompt-surface
- measurement
- prompt-tdd
topic: hidden information eval codescout help hinder recall precision
---

Status: approved design, not yet implemented. Opened 2026-08-23.
Work-stream log: `docs/trackers/prompt-surface-measurement-session-log.md`.

## The question

Does codescout help or hinder an agent doing real code archaeology? Every measurement
so far is about what codescout **costs** — 52,574 chars of tool surface, 97,690 chars
of guide corpus. Nothing measures what it **buys**. A compaction decision made on cost
alone is a decision made on half the ledger.

The eval answers one question with a counterfactual: on an identical task over an
identical codebase, is an agent with codescout better off than an agent with only
`Read` / `Grep` / `Glob` / `Bash`?

## Non-goals

- Not a benchmark against other tools. The comparison is codescout vs. the agent's
  native toolset, nothing else.
- Not a measure of which codescout *sub-surface* pays. That is the ablation work, and
  it needs this eval's fixture to exist first. Per-band recall (below) gives a first
  hint without being that study.
- Not a librarian eval. The librarian is 62.1% of codescout's tool-surface footprint
  and deserves its own scenario; see § Follow-on.

## 1. Fixture

A generator, `scenarios/hidden-info/gen_fixture.py`, emits the codebase. Seeded and
byte-reproducible so both arms compare on identical bytes; the seed is recorded in
the ground-truth file.

**Shape:** ~100 files, ~15–25k lines of order/billing domain logic, continuing the
vocabulary of the existing `eval-bins/fixture-project` (`pricing.py`, `inventory.py`).
Idiomatic Python — a reviewer should be unable to tell it was generated without being
told, or findings will not generalize.

**Why synthetic:** a real open-source repo is very likely in training data. A model
answering from memory rather than from tools would silently collapse the gap between
arms and make codescout look useless. Synthesis also makes ground truth exact rather
than best-effort.

**Scale rationale:** large enough that reading every file is expensive, not large
enough to be impossible. Brute force is deliberately *not* blocked — it is **priced**.
An arm that reaches full recall by reading 200 files scores high F1 and terrible
token efficiency, and that is a real, interpretable result rather than a rigged one.

### Ground truth — 12 sites in three bands

The task is a tax-rate change-impact sweep. Twelve sites genuinely read or set the
rate, four per band:

| Band | Mechanism | Expected discriminator |
|---|---|---|
| A | The literal `TAX_RATE` identifier appears at the site | `grep` finds these; both arms should clear this floor |
| B | One hop — the site consumes the rate through a parameter, from a caller that reads it | `references` / `call_graph`; a `TAX_RATE` grep misses the consumer entirely |
| C | Vocabulary drift — the concept is spelled `levy`, `surcharge_pct`, `duty_multiplier`, `_rate_bp` | `semantic_search`; grep finds these only if the agent guesses the synonyms |

**Plus 8 decoys** — sites that look relevant and are not: the word `tax` in a comment,
a `TaxReport` class that only formats an already-computed figure, a rate constant used
solely by a test fixture, a legacy function that is defined and never called.

Decoys are load-bearing, not flavour. Without them precision is trivially 1.0, F1
collapses to recall, and the metric rewards listing the whole repository.

### Ground-truth file

`ground_truth.json` is emitted **outside the fixture tarball**. The agent must not be
able to read the answer key — a fixture that ships its own solution measures nothing.

```json
{
  "seed": 20260823,
  "task_id": "tax-rate-change-impact",
  "sites": [
    {"id": "A1", "band": "A", "path": "src/billing/invoice.py",
     "symbol": "compute_total", "why": "reads TAX_RATE directly"}
  ],
  "decoys": [
    {"path": "src/reports/tax_report.py", "symbol": "TaxReport.render",
     "why": "formats an already-computed total; reads no rate"}
  ]
}
```

## 2. Arms and controls

| Arm | Tools | Answers |
|---|---|---|
| `hidden-cs` | codescout MCP only; native `Read`/`Grep`/`Glob` denied | performance with codescout |
| `hidden-native` | no MCP; `Read`/`Grep`/`Glob`/`Bash` | performance without it |

Native tools are denied in `hidden-cs` deliberately. It mirrors how codescout is
actually run here (companion plugin active, native hard-denied), and it keeps the
comparison a clean tool-set-A-vs-B rather than "does the agent fall back to grep".
The both-available configuration is a legitimate third arm later; it is not this
study.

### How the denial is actually enforced (F-7)

Reconnaissance after this spec was approved found the harness **cannot do this
today**. `SessionConfig` carries only `permission_mode` (default
`bypassPermissions`, `src/prompt_tdd/adapters/claude_code.py:51`), which governs
prompting, not tool availability. There is no allow/deny plumbing anywhere in
`src/prompt_tdd/`, and the plugin-free profile's `settings.json` is `{}`.

The capability exists one layer down. Claude Code 2.1.241 offers
`--disallowedTools`, `--allowedTools`, and `--tools` (`""` disables all built-ins).
So this is a passthrough, not a redesign: add a `disallowed_tools` field to
`SessionConfig`, read it in `cli.py:69`, and emit it at `claude_code.py:174` and
`:361` — the same four sites `permission_mode` already threads through.

**Prerequisite task.** The passthrough lands before phase 1, not during it.

**Use `--disallowedTools` with an explicit list, not `--tools ""`.** The latter is
documented as covering "the built-in set", which *implies* MCP tools survive it —
but that is read off a help string, not measured. The explicit deny-list does not
depend on the distinction.

**And enforce detection independently of enforcement.** The checker must fail any
`hidden-cs` run whose `tool_names` include a native file tool, as its own class
(`native-tool-used`), mirroring the `no-mcp-tool-used` veto already in
`check_nullctl.py`. This stays in place *after* the passthrough works, because a
passthrough that silently stops working looks exactly like compliance.

The reason both are required: every existing arm in `scenarios/surface-budget/`
restricts tools by prompt instruction alone ("Do not use Bash, Read, Grep, Glob,
Edit or Write"). Following that precedent here would leave `hidden-cs` enforced by
request, and a run that ignored the request would contaminate the arm without
moving recall, precision, F1 or tokens — none of which look at which tools produced
the answer.

Both arms: identical fixture bytes, identical prompt, identical turn cap, same model.

**Controls, both required before any result is believed:**

- **Positive control.** A variant whose prompt names one target file outright. Both
  arms must score ≥ 0.9 F1. An arm that fails it is broken, and its main-arm number is
  not a finding. The absence of a positive control was flagged as the gap that turns a
  null into theatre.
- **Noise floor.** The existing byte-identical `codescout-base` / `codescout-null`
  binary pair on the same task. Any split between them is a confound, and bounds the
  smallest difference worth interpreting.

## 3. Task and output contract

A change-impact sweep: *we are changing the tax rate — find every place that would
need to change and every place that reads the current value.*

The answer must end with a `## FINDINGS` section, one `path:symbol` per line, nothing
else in that section.

**Turn cap: 60 tool-using turns**, identical in both arms. See § 9 — whether the
harness can enforce this or it is a prompt instruction plus a post-hoc metric is
unresolved. Either way a run that exceeds the cap is reported with its turn count,
never silently dropped: excluding overruns would systematically delete exactly the
brute-force runs the token metric exists to catch.

This is a **scoring decision, not a UX one.** Every fuzzy-matching checker written in
this repo has failed the same way — substring presence scoring a denial as a pass,
a predicate demanding the wrong token. A constrained answer shape moves the checker
from judgment to set arithmetic, which is the only kind that has not yet fabricated.

**Matching rules.** The canonical form is repo-relative, forward-slash, no leading
`./`. The checker normalizes exactly three deviations — backslash separators, a
leading `./`, and an absolute path under the fixture root — and classifies anything
else as an unparseable line rather than guessing. Symbol match is exact and
case-sensitive. A file-level match with the wrong symbol does **not** count as found,
but file-level recall is reported separately so near-misses are visible rather than
silently absorbed.

**Malformed output is its own class** (`no-findings-block`), never zero recall. A
formatting failure must not masquerade as a capability failure — the same discipline
that `denied-the-fact` enforces in the null control.

## 4. Metrics

Per run:

| Metric | Definition |
|---|---|
| `recall` | \|found ∩ truth\| / 12 |
| `precision` | \|found ∩ truth\| / \|found\| |
| `f1` | harmonic mean of the two |
| `recall_band_{a,b,c}` | recall within each band |
| `recall_file` | file-level recall, ignoring symbol |
| `tool_calls`, `turns` | from the trace |
| `prompt_tokens` | cache-inclusive — input + cache_creation + cache_read |
| `output_tokens`, `cost_usd` | |
| `guidechars`, `guide_topics` | `hidden-cs` only |

**Primary comparison: F1 against prompt-tokens-to-answer, per arm per model.** Neither
alone decides anything. A step that raises F1 while tripling tokens is not a win, and
an arm that ties on F1 at a third of the tokens is.

**Per-band recall is the diagnostic that turns a verdict into a diagnosis.** "codescout
wins" is one bit. "codescout ties on band A and wins on band C" localizes the value to
`semantic_search`, which is directly actionable for compaction — it says the 62.1%
librarian footprint is not what is paying.

## 5. Sequence and gates

**Phase 0 — build (offline, free).** Generator, ground truth, checker, and the
checker's own unit tests. The checker is tested against hand-written adversarial
inputs before it ever scores a real run: a full-repo dump (precision must crater), an
empty findings block, a malformed block, a perfect answer, and an answer with the
right files but wrong symbols. Verify the exec bit with `ls -l` and say so — a checker
without it reports a clean 0/N indistinguishable from a genuine floor.

**Phase 1 — pilot.** N=2, Sonnet, both arms plus both controls. Roughly $2.

The pilot is a calibration and breakage check, **not** a significance test — N=2
cannot measure an effect. It proceeds to phase 2 only if all four hold:

1. Neither arm's mean F1 is < 0.15 (floored) or > 0.90 (ceilinged).
2. Both arms score ≥ 0.9 F1 on the positive control.
3. Noise floor: |F1(base) − F1(null)| ≤ 0.10.
4. The arms differ somewhere: |F1(cs) − F1(native)| ≥ 0.10, **or** band-C recall
   differs by ≥ 0.25.

Failing 1 or 4 means the fixture needs tuning (hop count, drift aggressiveness, repo
size), not that the finding is null. Failing 2 or 3 means something is broken; fix it
before spending more.

**Phase 2 — baseline.** 2×2: {`hidden-cs`, `hidden-native`} × {Sonnet, Opus}, N=8.
Pre-register in `docs/trackers/prompt-hamsa-audit-log.md` before the first run.

**Phase 3 — report.** Per-cell medians *and* per-run values. Never means alone: token
counts are long-tailed, and one brute-force run moves a mean without moving the
median. Read the `distinct` column before believing any tie.

## 6. Cost

The smoke arm cost $0.088 on a 30-line fixture at 5 turns. A 20k-line fixture under a
60-turn cap plausibly runs $0.50–2.00 per run on Sonnet, more on Opus. Thirty-two main-arm runs
lands somewhere in **$30–80**.

That figure covers phase 2's main arms only. Budget for the controls on top: the
positive control and the `base`/`null` noise floor run in phase 1, and are re-run
once per model in phase 2 as a guard against drift between phases — call it eight
extra runs, so plan **$40–100** end to end.

Set `max_cost_per_scenario` deliberately, and have the pilot report actual per-run cost
before committing to phase 2. If pilot cost lands at the top of the range, drop phase 2
to N=5 and say so in the report rather than quietly running fewer.

## 7. Risks

| Risk | Mitigation |
|---|---|
| Fixture reads as generated → findings do not generalize | Human skim before phase 2; the generator targets idiomatic code, not templated stubs |
| Agent brute-force reads everything | Not blocked, priced. Token count is a primary metric, so this shows up as a real result |
| Format non-compliance scored as incapacity | Own failure class; never folded into recall |
| Training-data contamination | Synthetic and seeded; no public provenance |
| N=8 too small for token variance | Report medians and per-run values; `--expect N` catches a truncated denominator |
| Rigged against grep | Band A exists precisely so grep has a fair floor; if codescout only wins on C, the report says so rather than claiming a general win |
| Opus spend overruns | Per-scenario cost cap; pilot reports real cost first |

## 8. Follow-on — the librarian eval

Requested 2026-08-23, deliberately scoped out of this design. The librarian is
subjectively the most helpful part of codescout and is 62.1% of its tool-surface
footprint, so it is simultaneously the biggest suspected value and the biggest
measured cost. Candidate axes:

- Does an agent with catalog access find a prior decision faster than one grepping
  `docs/`?
- Does provenance (`**Valid:**` / `**Rests on:**`) change whether an agent trusts a
  stale claim, or does it read past it?
- Does `librarian(context)` beat `semantic_search` for "what did we decide about X"?

It needs its own fixture — a repo with history, decisions, and deliberately stale
claims — so it is a sibling scenario, not a variant of this one.

## 9. Open questions

- Can the harness enforce a hard turn cap, or is it prompt-instruction plus a post-hoc
  metric? If the latter, runs exceeding the cap are reported, never silently excluded.
- Should band membership be revealed to the checker only, or also recorded in the run
  log? Recording it makes failures spot-readable; it cannot leak to the agent either
  way, since the checker runs after the response.
