1. **The mined candidates store post-correction context as `paragraph`, not the context containing the alleged positive sentence.**

   - Evidence: [mine_pairs.py](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:151) constructs only the new side of each hunk, and [line 183](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:183) stores it as `paragraph`. An independent scan of all 946 rows returned `positive_in_paragraph=30`, `twin_in_paragraph=605`, and `neither_in_paragraph=330`.
   - Impact: contextual training or auditing would usually omit the positive while exposing its correction, leaking the answer and invalidating the example. The held-out shingle filter also examines the corrected context instead of the context that would accompany the positive. The candidate file needs regeneration with separate pre- and post-correction contexts.

2. **The published cross-document shingle count is wrong: 25 document pairs overlap, not 20.**

   - Evidence: [mine_pairs.py:294-300](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:294) remembers only the first owner of each shingle, so for a shingle shared by A, B, and C it records A–B and A–C but misses B–C. Enumerating every combination returned `cross_group_pairs_implementation=20`, `cross_group_pairs_exact=25`, with five missed pairs. The incorrect 20 is published in [summary.txt:7](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/data/2026-09-24-rule-tell/stage2/summary.txt:7) and [the preregistration:364](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/phase1-local-classifier-preregistration.md:364).
   - Impact: the write-up understates cross-fold leakage constraints by five pairs; reusing this census logic during splitting would permit leakage between folds.

3. **The conclusion that no rule can reach 50 mined positives is not supported by the unlabelled candidate data.**

   - Evidence: all 946 committed rows have `rule: null`; 615 have no hint, while the heuristic produces 169 `closed_population` hints and 50 `count_unit` hints ([summary.txt:8-40](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/data/2026-09-24-rule-tell/stage2/summary.txt:8)). Nevertheless, [the preregistration:371](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/phase1-local-classifier-preregistration.md:371) applies the overall 10-row pair-quality estimate to hint counts and declares `count_unit` nearest and every rule below 50. The sample contains no rule labelling, and the miner explicitly describes hints as non-labels.
   - Impact: “the route cannot run on mined pairs alone” and the resulting synthetic-first pressure are premature. Only rule labelling can establish per-rule positive counts; genuine positives may occur among the 615 unhinted rows or be concentrated differently from the overall sample.

4. **The Stage 2 build is not reproducible from the committed checkout despite the README saying the script regenerates its input.**

   - Evidence: [README.md:78](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/data/2026-09-24-rule-tell/README.md:78) says the miner regenerates the omitted extract, but [mine_pairs.py:238](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:238) only reads `gitlog.patch`. Running the committed script failed with `FileNotFoundError`. Its documented extraction uses mutable `experiments`, while [line 38](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:38) reads held-out material from a hard-coded different checkout. A manual extraction pinned to `f828134a^` reproduced 1,038 candidates and 946 kept rows, but reported 4,330 scanned commits rather than the published 4,325.
   - Impact: the exact input revision and held-out corpus are not recorded, so the scan count cannot be reproduced and future reruns can silently build a different dataset.

5. **The claim that the “derive it” rule cannot be expressed as a sentence-level shape is an unsupported impossibility conclusion.**

   - Evidence: form 4 tested one widened wording, which falsely fired on two clean fixtures ([scoring doc:646-658](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/rule-tell-scoring-2026-09-23.md:646)). Line 658 then generalizes from that wording and five clean texts to “not expressible as a sentence-level shape.”
   - Impact: the experiment rejects this particular specification, not all possible sentence-level formulations. Treating it as an impossibility could wrongly terminate further selector designs.

6. **The filename-only document grouping already merges distinct source documents; it is not merely a hypothetical risk.**

   - Evidence: [mine_pairs.py:284-286](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:284) uses only the basename. The data contain 257 distinct paths but 250 groups. Most collisions are archive moves, but `docs/archive/old-trackers/bug-tracker.md` and `docs/issues/bug-tracker.md` are separate documents forced into one `bug-tracker.md` group. [The preregistration:378](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/docs/evals/phase1-local-classifier-preregistration.md:378) says this only “could in principle” happen.
   - Impact: “250 source documents” is actually 250 basename groups, and document-level folds or sampling will conflate unrelated sources.

7. **Score A report mode can falsely attribute an existing corpus to any selected model/form because the JSONL has no provenance header.**

   - Evidence: [phase1-span-selector.py:536-543](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-0/scripts/phase1-span-selector.py:536) prints the current CLI model/form before merely reading `--report`; rows contain no model, form, prompt hash, or run header. Reporting the Sonnet form-4q file with default arguments printed `judge: claude-haiku-4-5-20251001 ... form 4q` while reproducing the same totals.
   - Impact: a re-rendered result can be assigned to the wrong selector arm, undermining model/form comparisons even though the arithmetic remains unchanged.

Checks that came out clean:

- Independently recomputed form 4q as exactly 924 rows: 42 texts × 22 distinct rules, zero errors, and all 882 carried rows exactly matched form 3 after removing `carried_from`.
- Recomputed all form 3 → 4q bucket totals; they match the published 3/8, 2/9, 5/21, and 5/17 figures.
- Form 4 and form 4q gate logs support 6/8 failure and 6/6 pass respectively.
- The documented random seed reproduces the exact ten-row Stage 2 sample.
- Candidate totals, kinds, marker-source counts, and hint distributions match the committed JSONL.
- All 29 JSON/JSONL files parsed successfully, covering 8,636 JSONL rows.
- The two Python test modules ran 17 tests with no assertion failures; `phase2-score-dp1.py` emitted unclosed-file `ResourceWarning`s.
- Both archived issue paths exist, their former paths do not, and no old-path citations remain.
- `git diff --check 9c0d2505..898d3ea3` was clean.
- No Cargo or Rust tests were run, as requested.