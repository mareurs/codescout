## Findings

1. **The planned C1 evaluation selects its base arm on the supposedly held-out test set T.**  
   Evidence: T is declared primary and “never read during training or selection” at [phase1-local-classifier-preregistration.md:74](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/phase1-local-classifier-preregistration.md:74) and [line 83](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/phase1-local-classifier-preregistration.md:83), but C1 chooses “the best L-arm by T any-fire rate” at [line 98](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/phase1-local-classifier-preregistration.md:98).  
   Impact: C1’s T metrics would be test-selected and optimistically biased; T would no longer support the planned primary held-out conclusion.

2. **Stage 2’s claim that labels “come from construction” is unsupported for both proposed data sources.**  
   Evidence: [phase1-local-classifier-preregistration.md:53](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/phase1-local-classifier-preregistration.md:53) says a correction diff supplies the rule label and that every other generated sentence is negative for every rule ([lines 55–58](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/phase1-local-classifier-preregistration.md:55)). A diff identifies changed text, not which rule was violated or whether the corrected twin is clean; likewise, a generator being asked for one violation does not prove it introduced no other violations. Only 10% is audited, and up to 20% disagreement is accepted.  
   Impact: the proposed 1.5–3 week build can train and test on systematically wrong multi-label negatives; the recommended audit of the nine existing `partial` excerpts does not repair this labeling procedure.

3. **The Stage-2 holdout and Stage-4 gate still name eight gate texts even though the executable gate now has ten.**  
   Evidence: the explicit holdout list says eight texts at [phase1-local-classifier-preregistration.md:62](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/phase1-local-classifier-preregistration.md:62), and Stage 4 again specifies eight at [line 88](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/phase1-local-classifier-preregistration.md:88). The later amendment adds `clean-5` and `member` ([line 183](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/phase1-local-classifier-preregistration.md:183)), and the code constructs a ten-text gate at [phase1-span-selector.py:196](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/scripts/phase1-span-selector.py:196).  
   Impact: the two new fixtures can enter training or generation seeds and then be reused as gate cases, invalidating the trained-arm gate.

4. **The trained-arm Score B plan still prescribes the invalid `rtd8` checker.**  
   Evidence: [phase1-local-classifier-preregistration.md:99](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/phase1-local-classifier-preregistration.md:99) names `rtd8`; the clean-channel gate later returned NO/NO/NO on its positive fixture ([rule-tell-scoring-2026-09-23.md:398](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/rule-tell-scoring-2026-09-23.md:398)), and the campaign superseded it with `rtd8c` at [line 428](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/rule-tell-scoring-2026-09-23.md:428).  
   Impact: following the local-route preregistration literally makes RTD-8 unscoreable or revives the contaminated-channel result.

5. **The published 3/17 “recall” denominator includes authored positives that the implemented selector question explicitly defines as NO.**  
   Evidence: for example, the selector says `open_artifact` is NO when no basis is stated, `run_tool` is NO when no basis is stated or the text merely describes code, and `count_unit` is NO when unit and population are named ([phase1-span-selector.py:66](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/scripts/phase1-span-selector.py:66)). Yet the `partial` positives include RTD-5 with no stated basis, RTD-7 describing code paths, and RTD-16/19 with explicit units and populations. My independent recomputation confirmed all nine `partial` cases contribute misses to the reported 3/17.  
   Impact: 3/17 is not recall for one coherent classification task. In particular, the claimed family-level ceiling of 5/17 and the decision not to try a rule-family revision are not established; on the eight `yes` excerpts, the corresponding form-3 family reading reaches 4/8.

6. **RTD-9’s asserted “30-day retained source” premise is factually wrong for the records being counted.**  
   Evidence: the checker and documents say the `pika_observations` count is invalid because its underlying records have a 30-day horizon ([phase2-score-dp1.py:66](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/scripts/phase2-score-dp1.py:66)). The actual retention branch exempts every referenced `tool_calls` row from pruning ([src/usage/db.rs:332](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/src/usage/db.rs:332)), and the regression test explicitly establishes that `pika_observations` itself is untouched ([src/usage/db.rs:1721](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/src/usage/db.rs:1721)).  
   Impact: the RTD-9 correction and positive-control fact are false. The original “ever” claim may still exceed a one-machine census, but this experiment does not validate the published interpretation that the missing fact was a 30-day retention window.

7. **The phase-2 scorer can silently score the contaminated judge channel again.**  
   Evidence: it defaults to `~/.claude-kat` and checks only that credentials exist ([phase2-score-dp1.py:183](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/scripts/phase2-score-dp1.py:183), [line 212](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/scripts/phase2-score-dp1.py:212)); unlike the selector, it never rejects enabled plugins, hooks, or `CLAUDE.md`. Its output also does not stamp the config directory. The handoff acknowledges this at [rule-tell-scoring-2026-09-23.md:622](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/rule-tell-scoring-2026-09-23.md:622).  
   Impact: omitting one environment variable can reproduce the exact contamination that materially changed RTD-8 rates, while leaving a normal-looking successful score log.

8. **The phase-2 scorer accepts incomplete, duplicated, and errored replay batches as successful results.**  
   Evidence: [phase2-score-dp1.py:257](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/scripts/phase2-score-dp1.py:257) groups whatever rows are present, merely increments `errored`, and always returns 0 at [line 278](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/scripts/phase2-score-dp1.py:278). The saved pipeline guards only total line count, so ten duplicate or error rows pass ([score-b-s0.sh:31](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/data/2026-09-24-rule-tell/score-b-s0.sh:31)).  
   Impact: a future partial fork run can produce a plausible published rate and exit 0—the same failure that was fixed for Score A remains live in Score B.

9. **The committed evidence does not substantiate the claim that raw rows behind every result were saved.**  
   Evidence: [README.md:3](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/data/2026-09-24-rule-tell/README.md:3) characterizes the directory as the raw evidence, but `phase2-score-dp1.py` discards every per-row vote and prints only arm aggregates ([phase2-score-dp1.py:269](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/scripts/phase2-score-dp1.py:269)). The directory also contains neither the phase-0 Jev mutation-gate probability rows nor the ten DP1 Jev choices supporting “`none` on all 10”; `phase1-jev.jsonl` is the separate 21-pair corpus run.  
   Impact: the clean-channel phase-2 rates and those two Jev conclusions cannot be independently recomputed from the committed artifacts; a reviewer can only trust the saved aggregate logs.

10. **The stated 30% RTD-3 doc-write observability belongs to the superseded sample, not the fresh shipping sample.**  
    Evidence: the limits section reports 30% at [rule-tell-scoring-2026-09-23.md:237](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/rule-tell-scoring-2026-09-23.md:237). Counting the committed fresh `fork-rtd3r.jsonl` rows returned 6 of 10 arm-0 rows with `mcp__codescout__doc`; 3 of 10 is from `fork-rtd3-arm0.jsonl`, the earlier superseded-observable run.  
    Impact: the published limitation mixes samples and understates decision-point observability for the result that ships.

11. **The phase-1 chance comparator is slightly understated.**  
    Evidence: [rule-tell-scoring-2026-09-23.md:275](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/docs/evals/rule-tell-scoring-2026-09-23.md:275) reports 1/23, while scoring accepts either gold label for RTD-9 and RTD-15 ([phase1-rule-selection.py:82](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-primed-2/scripts/phase1-rule-selection.py:82)). Over the 17 `yes+partial` cases, random top-1 success is 19/(17×23) = 4.86%, not 4.35%. The adjacent comment also says three cases have two gold labels, but the mapping has two.  
    Impact: the baseline number is wrong, although the difference is too small to change the 24% exploratory conclusion.

## Checks that came out clean

- Verified HEAD is `a8835d06b41dd6901943a796ad2b18aa8abad933`; `git status --short` and `git diff --stat` were empty.
- Parsed all four Python files successfully with `ast.parse`.
- Parsed all 6,766 rows in all 25 JSONL files without an error.
- Recomputed the phase-0 reasoned tables from 2,850 unique rows; the positive, near-miss, and full-shape figures match the scoring document.
- Recomputed both 924-row Score A tables; each has exactly 42 texts × 22 unique rules, with no errors or incomplete groups, and the printed bucket counts match.
- Recomputed the Jev and Haiku phase-1A top-1/top-3/`none` counts; they match.
- Verified the L0 gate’s 220 probabilities, ranges, gold ranks, margins, and 4/10 result; they match.
- Verified all non-discarded replay files contain unique arm/run pairs and the registered run counts; the stripped API file has exactly the disclosed 17 error rows.
- Verified the scoped files’ hashes were unchanged across the review. No Cargo or Rust test command was run.