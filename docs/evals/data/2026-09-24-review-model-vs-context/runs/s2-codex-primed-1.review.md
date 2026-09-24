1. The conclusion that mined pairs cannot get any rule to 50 positives is unsupported because every candidate has `rule: null`, while the 10-row sample measures pair quality—not per-rule yield.

   - Evidence: [phase1-local-classifier-preregistration.md:368](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/phase1-local-classifier-preregistration.md:368) describes 3 genuine, 1 weak pair but assigns no rules; [mine_pairs.py:4](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:4) explicitly says hints are guesses, not labels. I counted 946 null labels, including 615 unhinted rows; `count_unit` has exactly 50 “any hint” matches.
   - Impact: multiplying the overall 30–40% pair-quality estimate into `count_unit`’s hints to get “about 15,” then concluding no rule can reach 50 ([preregistration:371](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/phase1-local-classifier-preregistration.md:371)), is invalid. A labelled or rule-stratified sample is required before deciding mined-only training is impossible or synthetic data is mandatory.

2. The proposed next build incorrectly replaces incident grouping with document grouping, although the controlling amendment requires both and the committed candidates contain correction chains crossing document groups.

   - Evidence: [phase1-local-classifier-preregistration.md:277](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/phase1-local-classifier-preregistration.md:277) requires all examples from an incident and the source document to share a fold; [line 378](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/phase1-local-classifier-preregistration.md:378) instead says “the document, not the incident.” An independent scan found three rows whose incident root belongs to a different path/document group, including `IC-14-guard-narrower-than-its-name.md → issue-clusters.md@0c5bab41b4`.
   - Impact: using only `doc_group` can split an original/correction chain between training and validation or calibration, causing leakage. Fold assignment needs connected components over both incident and source-document relationships.

3. “946 candidate pairs” is the wrong unit: nine committed candidates have no corrected twin.

   - Evidence: [mine_pairs.py:11](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:11) defines `note` candidates with `twin = null`; [summary.txt:4](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/stage2/summary.txt:4) reports 937 rewrites and 9 notes, while [README.md:75](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/README.md:75) calls all 946 pairs.
   - Impact: the actual structural population is 937 positive/twin candidates plus 9 one-sided notes. The published pair volume and available hard-negative count are overstated by nine.

4. The published cross-document shingle count is 20, but the committed candidates contain 25 document-group pairs sharing an 8-token shingle.

   - Evidence: [mine_pairs.py:294](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:294) retains only the first owner of each shingle, so for a shingle in A, B, and C it records A–B and A–C but misses B–C. I independently inverted all shingles to their complete owner sets and enumerated every pair: exact `25`, implemented algorithm `20`, five missing pairs. The wrong 20 is published at [summary.txt:7](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/stage2/summary.txt:7) and [preregistration:364](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/phase1-local-classifier-preregistration.md:364).
   - Impact: the planned cross-fold leakage cleanup is understated, and the current algorithm cannot verify that all cross-group collisions were resolved.

5. Incident origins are not computed “per path” as claimed; `first_seen` is keyed only by sentence text.

   - Evidence: [mine_pairs.py:168](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:168) uses `sentence hash → commit`, and [line 274](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:274) reuses that global origin despite the “per path” comment. Candidate [mined-candidates.jsonl:618](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/stage2/mined-candidates.jsonl:618) attributes an `IC-10...` sentence to `5e5383e3a8`; `git show --name-only 5e5383e3a8` shows that commit touched only `docs/trackers/issue-clusters.md` and a test. Checking every committed incident found 18 claimed path/origin pairs where the origin commit did not touch that path.
   - Impact: “513 incidents” is a count of faulty identifiers, not established source incidents, and incident-based grouping can merge or split the wrong units.

6. The miner is not reproducible from the committed checkout and does not regenerate the omitted patch as the README claims.

   - Evidence: [README.md:78](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/README.md:78) says the script regenerates the extract, but [mine_pairs.py:238](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:238) only opens absent `stage2/gitlog.patch`; running it exits 1 with `FileNotFoundError`. Its documented command targets moving `experiments` ([line 8](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:8)), and corpus imports use the author-specific `/home/marius/work/claude/codescout` checkout ([line 38](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py:38)). The published 4,325-commit input matches history at `938e6d0c`, whereas `f828134a^` already contained 4,330 matching commits. Running the documented command now yielded 4,350 commits, 1,048 found, and 947 kept.
   - Impact: neither the input revision nor the build is stable; a re-run silently answers for a different history and potentially a different checkout’s selector/corpus.

7. Form 4 establishes that this particular `count_unit` wording lacks precision, not that the law is impossible to express at sentence level.

   - Evidence: the evidence is two clean fixtures and three runs each in [form4-gate.txt:16](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/data/2026-09-24-rule-tell/form4-gate.txt:16), but [rule-tell-scoring-2026-09-23.md:658](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/docs/evals/rule-tell-scoring-2026-09-23.md:658) concludes the “derive it” distinction “is not expressible as a sentence-level shape.”
   - Impact: that impossibility claim overgeneralizes one tailored wording and can wrongly rule out narrower specifications or additional sentence/context features. The supported conclusion is only that `SPECS_F4["count_unit"]` failed this gate.

8. `--report` can falsely relabel an existing Score A file with the current default model, form, and judge channel.

   - Evidence: [phase1-span-selector.py:536](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/scripts/phase1-span-selector.py:536) prints live CLI configuration before [line 542](/home/marius/work/claude/codescout.worktrees/rmvc-s2-codex-primed-1/scripts/phase1-span-selector.py:542) reads the report rows, while those rows contain no model/form header. Reporting the committed Sonnet/form-4q file with defaults printed `claude-haiku-4-5-20251001`, form `2b`, and the dirty `~/.claude-kat` config, although its recorded run was Sonnet/form 4q on the clean channel.
   - Impact: a later re-report can publish correct arithmetic under false experimental provenance; the provenance must come from a persisted header or explicit validated arguments.

Checks that came out clean:

- The two relevant Python test modules passed all 17 tests; no model calls were made.
- Form 4’s gate arithmetic reproduced as 6/8 with zero errored rows; form 4q reproduced as 6/6.
- Form 4q contains exactly 924 unique `(case, side, rule)` cells: 42 texts × 22 rules, zero errors.
- All 882 carried non-`question_asked` cells are identical to form 3 after removing `carried_from`; all 42 `question_asked` rows are fresh.
- Independent form-4q scoring reproduced 3/8, 2/9, 0/4 gold recall; 5/21 negative texts with any fire; and overall recall 5/17.
- The committed form4q readout and partial-audit output reproduce exactly.
- The 946 retained candidates currently have no 8-token collision with the committed held-out corpus and none comes from a blacklisted campaign path.
- The checkout was clean before and after review.