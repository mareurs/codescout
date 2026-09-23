---
id: '06bcdbaf7583c526'
kind: eval
status: active
title: Rule-tell scoring — first run, 2026-09-23 (provisional, single judge)
tags:
- eval
- rule-tells
- precision
- results
- provisional
- classifier
topic: rule-tell-detection
---

**Valid:** dated 2026-09-23

This records the first scoring run of the five rule-tell detector prompts against the control corpus. The design is in `docs/evals/rule-injection-timing-preregistration.md` (`11f039dc91b862ec`) and the corpus is `docs/evals/rule-tell-controls.md` (`cfa0d5bc1bdeacab`).

**Read the scope first:** the run is **single-family and provisional.** The pre-registration names a cross-family panel. The Gemini leg was unavailable (its key returns `API_KEY_INVALID`), so every verdict below comes from one judge, no divergence could be detected, and no row could be withheld.

## How to reproduce

The pipeline has three stages, and each is a script in this repo:

```
scripts/blind-rule-tell-tasks.py     corpus -> 285 blind tasks (5 prompts x 57 passages)
scripts/score-rule-tell-prompts.py   --mode mutation (gate), then --mode score
scripts/analyse-rule-tell-run.py     scored rows + corpus labels -> the tables below
```

The judge is `claude-haiku-4-5-20251001` through `prompt-engineering:PanelJudge`. The scorer uses `--judges claude`, which stamps `judges: claude` on every row so the analyser can refuse panel-only conclusions. The scorer forces the **text** judge path, for the reason in § *The first run was void*.

The scored rows live in the session scratchpad and are not committed: they carry passage text and judge reasoning, and they would be re-derived on any re-run anyway. The in-repo analyser reproduced the recorded output byte for byte before any number here was written.

## The mutation gate passed before any corpus row was spent

The gate is fifteen hand-written rows (per prompt: a clear YES, a clear NO, and a realistic near-miss), following `prompt-engineering:skill-eval-playbook` `L-13`. The poles split on **5 of 5** prompts.

The near-miss column is not graded by the gate. One cell still earned its place: RTD-8 scored **1.00** on *"Nothing in the scheduler reads `retry_budget`. A workspace-wide grep returns only its definition."* That text meets both of RTD-8's own NO conditions, since the negative is scoped and the search spans the claim.

## The first run was void

The first 285-row pass used the Anthropic structured-output path and produced no usable verdicts:

- 72% of rows had empty reasoning.
- 52 rows had scores no judge would give, such as `1e-121`, ten rows at `1e-16`, and one at `1.0018`, which is outside the declared range.
- The correlation had no exceptions. All 80 rows with reasoning had sane scores, and every degenerate score was on a row without reasoning.

The cause is in `prompt-engineering`, not here, and it is harness-wide. `Verdict.reasoning` has a default, so the generated schema does not require it, and the model leaves it out: 29 of 32 of the harness's own stock rubrics came back without reasoning. Filed as `prompt-engineering:docs/issues/2026-09-23-structured-judge-verdict-omits-reasoning-and-scores-unbounded.md` (`prompt-engineering:98c5431`).

Two positives that "failed" in the void run passed once it was re-scored. The first run's gate verdict was an artifact of the judge path, not a property of the prompts.

## Results — text path, 285/285 scored, 0 errored, every row with reasoning

### Positive gate: 4 of 5

| positive | prompt | verdict |
|---|---|---|
| `CTL10-8` | RTD-10 | YES 1.00 |
| `CTL3-6` | RTD-3 | YES 0.75 |
| `CTL8-6` | RTD-8 | YES 1.00 |
| `CTL9-4` | RTD-9 | YES 1.00 |
| `CTLX-5` | contradiction | **NO 0.00** |

`CTLX-5`'s NO is correct for what the judge was shown. The corpus stores the passage as two fragments, and the blinding extractor joins them with `[…]`, which drops the material the contradiction depends on. The judge's own reasoning says so: *"The excerpt provided is too brief and fragmented to evaluate against the criterion."* So the **contradiction prompt is unscoreable on this corpus as extracted**. The other four prompts cleared their gates.

### Precision — a YES on a control is a false positive

| prompt | controls | YES | full-shape | near-miss |
|---|---|---|---|---|
| RTD-10 | 12 | **9** | 8/10 | 1/2 |
| RTD-3 | 10 | 6 | 6/10 | — |
| RTD-9 | 10 | 6 | 5/6 | 1/4 |
| RTD-8 | 10 | 3 | 3/8 | 0/2 |
| contradiction | 10 | 4 | not interpretable | |

### Cross-talk — how often a prompt fires on passages outside its own shape

RTD-8 15/46 (33%) · RTD-10 14/44 (32%) · RTD-3 9/46 (20%) · RTD-9 2/46 (4%) · contradiction 1/46 (2%).

## Against the registered predictions

- **RTD-10: confirmed on the numeric threshold, and not on the strongest reading.** The registration predicted a YES on at least 3 of the 12 controls, and the observed count is 9. The two near-misses that were meant to discriminate split: `CTL10-13` drew YES (0.70) and `CTL10-11` drew NO. So the registered strongest reading, a YES on both, did not happen.
- **RTD-9: reported split, as registered.** It fired on 5 of 6 full-shape controls and on 1 of 4 near-misses. A pooled 6/10 would have hidden that the prompt is weakest exactly on the controls built to look like its tell.

## What this does not establish

- **Nothing is cross-family.** The calibration section of the analyser refused to run, because every spread is 0.0 when there is one judge, and a clean-valley verdict would then be an artifact of the setup. The panel's own calibration also predates the structured path and was measured on the text path (see the `prompt-engineering` bug file).
- **The cells are small.** Each prompt has 10–12 controls, and the near-miss sub-cells hold 2–4.
- **The contradiction prompt has no result.** Measuring it needs whole documents, or passages that keep the material between their fragments. That is a corpus decision and has not been made.
