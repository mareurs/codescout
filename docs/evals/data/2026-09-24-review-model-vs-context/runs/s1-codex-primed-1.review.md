1. The evaluation labels one identical excerpt both clean and violating.

   - Evidence: [phase1-rule-selection.py](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/scripts/phase1-rule-selection.py:294) assigns every negative `none`, while the source corpus explicitly warns that RTD-16’s negative is RTD-19’s positive and must not be treated as clean ([rule-tell-detection.md](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/rule-tell-detection.md:891)). My parser confirmed `RTD-16 negative == RTD-19 positive: True`.
   - Impact: the reported clean-side figures are wrong: S0 form 2b is 2/20, not 2/21; form 3 is 5/20, not 5/21; Jev’s corrected-text figure is 30/60, not 30/63; and Haiku’s is 1/100, not 1/105. More importantly, Stage 2 repeats the falsified assumption that a correction twin is automatically a hard negative ([phase1-local-classifier-preregistration.md](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/phase1-local-classifier-preregistration.md:53)).

2. The active local-route plan still specifies the invalid `rtd8` checker and stale contaminated-channel baselines.

   - Evidence: the plan names `rtd8` for Stage-4 Score B and cites RTD-8 baselines 5/10 and 0/10 ([phase1-local-classifier-preregistration.md](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/phase1-local-classifier-preregistration.md:28), [line 99](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/phase1-local-classifier-preregistration.md:99)). That checker later failed its clean-channel gate 0/3 ([rule-tell-scoring-2026-09-23.md](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/rule-tell-scoring-2026-09-23.md:398)); the valid replacement is `rtd8c`, with clean fork baselines 9/10 and 2/10 ([line 439](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/rule-tell-scoring-2026-09-23.md:439)).
   - Impact: following the active Stage-4 plan would either stop at a checker gate known to fail or compare a new clean-channel arm with obsolete contaminated-channel numbers, producing a wrong ship decision.

3. Score A’s “completeness” guard accepts a corpus missing entire texts.

   - Evidence: [phase1-span-selector.py](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/scripts/phase1-span-selector.py:333) checks only whether each observed text has 22 rule rows; it never checks that all 21 cases and both sides exist. Running `report_corpus` on just RTD-1’s 22 negative rows returned `1 texts, 22 rows, 0 incomplete` and exit 0. The committed probe records the same behavior ([p1s-offline.txt](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/p1s-offline.txt:16)).
   - Impact: an interrupted run ending between texts can still be published as “0 incomplete” with biased rates. The committed 924-row runs are complete, but the claimed fix does not protect future Score-A runs.

4. The committed data cannot substantiate that the decisive phase-2 re-scores were made on the clean channel or reproduce their row verdicts.

   - Evidence: `phase2-score-dp1.py` defaults to the known contaminated `~/.claude-kat` profile ([phase2-score-dp1.py](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/scripts/phase2-score-dp1.py:212)), does not print the selected config, and discards each row’s three votes after aggregating them ([line 269](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/scripts/phase2-score-dp1.py:269)). Accordingly, [api-score-rtd8c.txt](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/api-score-rtd8c.txt:1) contains only gate and aggregate counts. The README confirms that startup logs and judge configs were omitted ([README.md](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/README.md:64)).
   - Impact: the final “all three rules ship on the clean channel” conclusion cannot be independently checked from committed evidence. A rerun would create new stochastic judgments, not audit the original ones; this matters because the campaign itself observed channel shifts of several rows.

5. The phase-2 scorer has no completeness or uniqueness check for arms and runs.

   - Evidence: it loads whatever rows exist, optionally filters them, counts present rows, and returns 0 even with missing, duplicated, or errored replays ([phase2-score-dp1.py](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/scripts/phase2-score-dp1.py:257)). It never verifies ten unique runs for every requested arm.
   - Impact: a truncated future Stage-4 input can yield a plausible `x/9` or `x/8` rate indistinguishable from the campaign’s legitimate post-treatment unobservability. Current committed phase-2 inputs did contain ten unique rows per registered arm.

6. Stage 2 does not require correction/synthetic twins to remain in the same train, validation, or calibration fold.

   - Evidence: the plan constructs minimal-edit pairs ([phase1-local-classifier-preregistration.md](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/phase1-local-classifier-preregistration.md:53)) and freezes three folds ([line 70](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/phase1-local-classifier-preregistration.md:70)), but specifies source-document grouping only for T and an 8-token filter only against held-out texts. The committed research recommendation explicitly called for incident-grouped splits ([jev-oss-clones-research.md](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/jev-oss-clones-research.md:246)).
   - Impact: a positive and its near-identical corrected twin may land across training and validation/calibration, inflating model selection, thresholds, and calibration. The split rule must be fixed before the planned data build is frozen.

7. The author summary understates adopted S0’s silence by conflating it with the rejected form-3 ablation.

   - Evidence: the committed table reports form 2b—the retained S0—as 13 silent positives out of 17, while form 3 has 10/17; form 3 was explicitly not adopted ([rule-tell-scoring-2026-09-23.md](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/rule-tell-scoring-2026-09-23.md:506), [line 515](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-1/docs/evals/rule-tell-scoring-2026-09-23.md:515)). My independent row count reproduced 13 versus 10.
   - Impact: “S0 fires no rule on 10 of 17” is false for the selected baseline; its actual silence rate is 13/17. The proposed review of the nine `partial` excerpts remains relevant, but it does not account for all of adopted S0’s silence.

Checks that came out clean:

- HEAD was `a8835d06b41d`; the worktree was clean when review finished.
- All 25 committed JSONL files—6,766 rows—parsed successfully.
- Expected `(case, side, run/rule)` and `(arm, run)` keys were unique in the principal phase-0, phase-1, phase-2, and fork datasets.
- The two S0 corpus files each contain 924 rows, 42 observed texts, no errored rows, and 22 rule rows per observed text.
- Independent aggregation reproduced the documented S0 form-2b and form-3 hit/fire counts, apart from the invalid clean denominator above.
- The 2,850 phase-0 reasoned rows are complete, unique by task/run, and all carry reasoning; the four reported positive-gate rates reproduced.
- The L0 gate contains 220 probabilities; its 4/10 gate, 3/3 span gate, ranges, ranks, and reported margins reproduced.
- Registered phase-2 raw inputs contain ten unique rows per arm; the discarded stripped file contains exactly 17 error rows.
- Raw-row observability denominators match the committed phase-2 aggregate logs.
- All four scoped Python scripts parsed successfully with `ast.parse`.
- No network, model, Cargo, or Rust-test calls were made.