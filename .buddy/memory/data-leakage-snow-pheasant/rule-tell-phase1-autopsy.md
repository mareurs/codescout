---
specialist: data-leakage-snow-pheasant
scope: project
slug: rule-tell-phase1-autopsy
created: 2026-09-25
updated: 2026-09-25
tags: [rule-tell, fine-tuning, autopsy, preregistration]
---

**Lesson:** In this repo, the rule-tell local-classifier campaign is where my training-integrity lessons were paid for. Before reviewing any follow-up (phase 1b or later), I read its evidence first rather than re-deriving it.

**Why:** On 2026-09-25 phase 1 stopped at the Stage-4 gate (L2-QWEN 3/8, L1-MBERT 1/8). Post-stop diagnostics — registered before they ran — showed a clean permutation null (val AUC 0.498), a seed-unstable recipe (second seed val AUC 0.525), and token-separable pairs (12/14 rules ≥ 0.9). Four research agents then traced the failures to an off-recipe optimiser setup, no rule text in the input, and edit artifacts in the pairs.

**How to apply — where the evidence lives:**
- `docs/evals/phase1-local-classifier-preregistration.md` — § *Stage 4 — gate results*, its Codex correction, § *Diagnostics after the stop* and § *Diagnostics — results*.
- `docs/evals/phase1b-local-classifier-preregistration.md` — the unregistered next-attempt draft, with its revision history.
- `docs/research/2026-09-25-phase1-training-research-synthesis.md` — the four research reports, each claim marked verified / cited / derivation.
- `docs/evals/data/2026-09-24-rule-tell/phase1b/diagnostics/` — the raw diagnostic outputs.
- `docs/issues/2026-09-25-encode-units-drops-sentence-leading-space.md` — the tokenisation defect both arms carried.
