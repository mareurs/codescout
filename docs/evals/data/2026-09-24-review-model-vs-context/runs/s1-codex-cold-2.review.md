1. **Defect:** Stage 2’s proposed training labels are not valid multilabel ground truth: a correction diff identifies changed text but not which rule it violates, while the plan labels every other sentence negative for every rule.

   **Evidence:** [phase1-local-classifier-preregistration.md:53](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/phase1-local-classifier-preregistration.md:53) claims labels come from construction; lines 55–57 infer a rule label from a diff and blanket-label other sentences. Yet [phase1-rule-selection.py:75](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/scripts/phase1-rule-selection.py:75) explicitly supports multiple gold rules, including RTD-9 and RTD-15 at lines 82–84.

   **Impact:** Stage 2 will inject false negatives into every per-rule head and potentially attach the wrong rule to mined corrections, invalidating calibration and the planned Stage 3 result unless full multilabel annotation is added.

2. **Defect:** `report_corpus` is neither population-complete nor a valid implementation of the planned T “claim on target” metric.

   **Evidence:** [phase1-span-selector.py:333](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/scripts/phase1-span-selector.py:333) checks only observed `(case, side)` groups, never the expected case set; the committed control demonstrates that a file containing only one complete 22-rule text exits 0 at [p1s-offline.txt:16](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/data/2026-09-24-rule-tell/p1s-offline.txt:16). Its localization logic also hard-codes the old 21-case eval set and treats a quote absent from that correction as on-target ([phase1-span-selector.py:355](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/scripts/phase1-span-selector.py:355)); an unseen T case therefore receives an empty comparison string and every gold-rule quote counts as localized. This conflicts with the promised T metric at [phase1-local-classifier-preregistration.md:89](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/phase1-local-classifier-preregistration.md:89).

   **Impact:** A truncated Score A/T file can pass, and T’s primary claim-localization rate can be reported as perfect without comparing the selected sentence to T’s actual gold span.

3. **Defect:** The active Stage 4 protocol still specifies two superseded instruments: the old 8-text gate and the known-invalid `rtd8` checker.

   **Evidence:** [phase1-local-classifier-preregistration.md:88](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/phase1-local-classifier-preregistration.md:88) specifies eight gate texts, although the amendment says the current fair gate is ten at line 253; line 99 specifies `rtd8`, although it failed its clean-channel gate 0/3 ([rule-tell-scoring-2026-09-23.md:398](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/rule-tell-scoring-2026-09-23.md:398)) and was replaced by `rtd8c` at line 428.

   **Impact:** Following the preregistration literally would test trained arms with an easier, incomplete gate and make RTD-8 unscoreable or score it with a rejected checker.

4. **Defect:** `phase2-score-dp1.py` can silently run on the contaminated default judge profile and records neither judge configuration nor per-replay votes.

   **Evidence:** [phase2-score-dp1.py:212](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/scripts/phase2-score-dp1.py:212) defaults to `~/.claude-kat`; unlike the selector, it performs no cleanliness check. Lines 269–270 retain votes only in memory, and lines 271–277 emit aggregates. The purported clean artifact [api-score-rtd8c.txt:1](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/data/2026-09-24-rule-tell/api-score-rtd8c.txt:1) contains no model/config provenance or row decisions. The scoring document itself acknowledges the missing guard at [rule-tell-scoring-2026-09-23.md:620](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/rule-tell-scoring-2026-09-23.md:620).

   **Impact:** Omitting or mistyping `JUDGE_CONFIG_DIR` produces a plausible contaminated “clean” score, and the committed evidence cannot establish which channel produced the ship verdicts or independently recompute their counts.

5. **Defect:** The committed evidence omits the mutation-gate rows on which the Phase 0 corpus conclusions depend.

   **Evidence:** The result claims reasoned Haiku passed 5/5 at n=10 and reports Jev’s five-prompt gate at [rule-tell-scoring-2026-09-23.md:32](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/rule-tell-scoring-2026-09-23.md:32) and line 91. But the Phase 0 manifest lists only the 285-task corpus files at [README.md:5](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/data/2026-09-24-rule-tell/README.md:5); `find docs/evals/data/2026-09-24-rule-tell -type f` returned no reasoned, native, or Jev mutation-gate artifact.

   **Impact:** The critical evidence that these judges were not constant-YES/NO classifiers cannot be checked from the committed campaign data, contrary to the README’s claim that this directory is the evidence behind the results.

6. **Defect:** The published “chance on this menu is 1/23 (4%)” baseline is too low because the scorer accepts multiple gold options for some cases.

   **Evidence:** [rule-tell-scoring-2026-09-23.md:247](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/rule-tell-scoring-2026-09-23.md:247) gives the 23-option menu and line 275 states 1/23. Recomputing from `GOLD` found two—not the comment’s claimed three—multi-gold cases, RTD-9 and RTD-15 ([phase1-rule-selection.py:75](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/scripts/phase1-rule-selection.py:75)). Across the 17 scored `yes`/`partial` positives there are 19 accepted gold slots, so uniform top-1 chance is `19/(17×23) = 4.86%`, about 5%.

   **Impact:** The selector’s advantage over chance is modestly overstated, and both the code comment’s gold-count claim and the published baseline are wrong.

7. **Defect:** The reported 5/17 family recall is not an upper bound, despite being presented as one.

   **Evidence:** [rule-tell-scoring-2026-09-23.md:519](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/rule-tell-scoring-2026-09-23.md:519) defines a post-hoc four-rule family and line 528 says no fairer grouping can do better. Direct aggregation of `p1s-S0f3-corpus.jsonl` found seven of the 17 detectable positives fire some rule: three gold hits plus wrong-rule-only fires on RTD-4, RTD-9, RTD-11, and RTD-20. Thus a broader post-hoc merge can reach 7/17.

   **Impact:** The published 5/17 “ceiling” and handoff characterization are false. The immediate ≥0.5 conclusion happens to survive because the actual absolute ceiling, 7/17, is still below 0.5.

8. **Defect:** The timing preregistration’s headline status falsely says nothing has run and every number is a pre-sample commitment.

   **Evidence:** [rule-injection-timing-preregistration.md:18](/home/marius/work/claude/codescout.worktrees/rmvc-s1-codex-cold-2/docs/evals/rule-injection-timing-preregistration.md:18) says “registered, nothing run,” while its own amendment says it was registered after an arm-0/arm-3 pilot at line 158 and publishes that pilot’s 2/3 and 0/3 outcomes at lines 169–174.

   **Impact:** A reader can incorrectly treat the entire document as prospectively registered; the amendments disclose the history, but the document’s controlling status claim contradicts it.

Checks that came out clean:

- Confirmed `HEAD` is `a8835d06b41dd6901943a796ad2b18aa8abad933`; working tree and diff remained clean.
- Parsed every committed JSON/JSONL record successfully and AST-parsed all four scoped Python scripts.
- Verified unique expected arm/run or case/side/rule keys in the principal datasets.
- Recomputed all Phase 0 positive and diagonal-control rates; they match the scoring document.
- Recomputed Phase 1A Jev/Haiku rates, S0 form-2b and form-3 Score A tables, and L0 probability/rank summaries; they match.
- Confirmed 17/30 stripped API rows are errors, all current replay arms have their stated row counts, and both `e2s` injection files align exactly by run with their forked replays.
- `bash -n` passed for `score-b-s0.sh`.
- Cargo and Rust tests were not run, as requested; no model, network, or API calls were made.