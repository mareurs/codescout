I found seven defects. The form 4 and form 4q headline numbers all reproduce exactly from the committed rows. The problems are in the Stage 2 conclusions and in how a few results are read.

One disclosure first. Running the miner imported the selector from inside the checkout, and Python wrote `scripts/__pycache__/` (three `.pyc` files, timestamped 22:07:38, the time of my run). The directory is gitignored and `git status` shows nothing else changed. I left it alone because the rules also forbid deleting files; it is safe to remove.

## Findings

**1. The Stage 2 stopping-rule verdict contradicts its own arithmetic.**
- **(a) Defect:** the document says no rule reaches the ≥50-positive bar from mined pairs and that `count_unit` is nearest, but `closed_population` has 169 hints, and 169 × 0.30–0.40 = 51–68, which clears the bar.
- **(b) Evidence:**
  - `docs/evals/phase1-local-classifier-preregistration.md:371` works `count_unit` as 50 raw hints × the sample precision ≈ 15.
  - The same method gives 51–68 for `closed_population`. Its any-hint count of 169 is in `stage2/summary.txt` and matches the JSONL.
  - Line 369 dismisses that hint because it "only matches all / every / none". That is a separate, unmeasured discount, applied silently to one rule.
  - The 30–40% is also the rate of genuine *pairs*, not the rate at which a hint names the right rule. Nobody has measured the second number for any rule.
- **(c) Impact:** line 114's stopping rule says to continue on mined pairs alone if any rule reaches 50, otherwise stop. So "the route cannot run on mined pairs alone" decides the next step, and the operator's labeller-versus-synthetic choice is framed on it. The honest statement is that rule-level precision is unknown.

**2. "About 30–40% genuine" is a point estimate from 10 rows, given without an interval.**
- **(a) Defect:** the range comes from a 10-row sample and is then multiplied into later numbers as if it were measured.
- **(b) Evidence:**
  - `preregistration.md:368` bases it on 3 genuine and 1 weak out of 10.
  - Wilson 95% intervals: 3/10 gives 10.8–60.3%; 4/10 gives 16.8–68.7%.
  - This file requires Wilson intervals for its own unknown-cell audit.
  - In the same sample, all 3 rows hinted `closed_population` are mismatched or fragments, which would support finding 1's discount. The document does not cite it.
- **(c) Impact:** the "about 15" and every rule-reach estimate carry roughly a sixfold uncertainty that the text does not show.

**3. The held-out filter does not cover the Score A cases' own source documents and incidents, and one leaked candidate survives.**
- **(a) Defect:** `HELD_OUT_DOC_RE` (`stage2/mine_pairs.py:54`) drops only this campaign's documents, not the source documents of the RTD cases.
- **(b) Evidence:**
  - One kept row comes from `e365a6b3`, which is the RTD-12/13 correction commit, in the same ADR.
  - Its incident is `…@ddfce06545`, the commit that wrote RTD-12's hedge. Its paragraph is the same Confidence block: `e365a6b3` removes RTD-12's hedge sentence at diff line 100.
  - Its positive states "15 params-backed and 16 prose-backed trackers", the same population behind RTD-16 and RTD-21.
  - The 8-token shingle filter missed it, because the new-side paragraph no longer contains the removed hedge.
  - Separately, 4 kept rows share 8-token shingles with the API-route DP1 drafts (`phase2-dp1-*.jsonl`, `phase2-pilot.jsonl`). The miner only reads `fork-*.jsonl`. This one is ambiguous, because the registration's list names only the "fork drafts".
- **(c) Impact:** only one row today. But the status note says the build applies the filter "against every held-out source" and the amendment's rule that one incident or source document goes into one fold, and the miner is structurally blind to this class. It needs fixing before a freeze.

**4. The form 4 write-up leaves out form 3's baseline on clean-4.**
- **(a) Defect:** clean-4 was already firing both rules under form 3, and the form 4 section does not say so.
- **(b) Evidence:**
  - `form3-gate.txt:17`: clean-4 fired `[['count_unit','scope_instant']]` in 1 of 3 runs under form 3, with the specs *not* widened.
  - The scoring doc reads clean-4's failure as a cost of widening (line 653). At line 659 it says "widening the rest made the judge apply it", a causal claim that one run per form cannot support.
- **(c) Impact:** clean-4 moved from 1/3 to 3/3 against a 2-of-3 threshold, so it was already borderline. The clean evidence is clean-3, which moved from 0/3 to 3/3. The conclusion that widening `count_unit` costs precision survives. The `scope_instant` attribution is weaker than stated.

**5. RTD-12 "quoted on the audit's own sentence" mixes up the audit's two quotes.**
- **(a) Defect:** the sentence form 4q fired on is the one the audit recorded as the caveat, not as the violation.
- **(b) Evidence:**
  - `rule-tell-scoring-2026-09-23.md:680` makes the claim.
  - The fired claim is the sentence holding the audit's `quote2`, which it logged as (b), the outside condition.
  - The audit's (a) quote is "So id allocation already survives a catalog loss" (`partial-audit-verdicts.json`).
- **(c) Impact:** small. The fired sentence does carry the wrong `append_entry.rs:294-324` citation, which the fix removed, so the fire is defensible. The wording overstates how exactly the fire matched the audit.

**6. The scorer's `question_sha256` does not cover the whole checker.**
- **(a) Defect:** the header hash does not support the handoff's claim that a later checker change "can be traced row by row".
- **(b) Evidence:** `scripts/phase2-score-dp1.py:289` hashes only `rule["question"]`. The judge prompt also appends `TAIL` (`:223`) and uses `SubscriptionJudge.SYSTEM`. The gate depends on the `--corrected` file and `extra_gate` texts, and none of these is hashed.
- **(c) Impact:** a change to any of them leaves the hash the same, so the rows cannot be tied to the checker that produced them.

**7. The committed analysis scripts read the main checkout by absolute path, not the committed tree.**
- **(a) Defect:** the scripts hardcode `ROOT = /home/marius/work/claude/codescout`.
- **(b) Evidence:** `form4q-readout.py:4`, `partial-audit-check.py:4` and `stage2/mine_pairs.py:38`. I had to rewrite `ROOT` to reproduce the miner from this worktree.
- **(c) Impact:** a re-run silently uses whatever selector code and corpus are in the main checkout at that moment, so the reproduction evidence depends on another tree's state.

**Also, a test gap (lower severity).** None of the 17 Python tests covers the form wiring in `main()`: the `SPEC_FORMS` rebinding for `4`/`4q`, adding `GATE_F4`, the gate's n/a and `applicable` logic, and the refusal of `--corpus --rules` without `--carry`. A grep of `tests/` for `SPEC_FORMS|4q|GATE_F4|applicable` finds nothing. So a mutation mapping `"4q"` to the unwidened `SPECS` would pass the suite. `CarriedRuleSubset` calls `corpus()` directly, which bypasses `main()`.

## Checks that came out clean

- **Form 4q rows:** 924 rows, no duplicate keys. There are 882 carried rows (21 rules × 42 texts), each byte-identical to form 3's row, plus exactly 42 newly judged `question_asked` rows.
- **Form 4q Score A table:** recomputed independently, and it matches (yes 3/8 → 3/8 with only-gold 2/8 → 1/8; partial 0/9 → 2/9; negatives 5/21 → 5/21). Recall 3/17 → 5/17 is correct.
- **Q1 and Q2:** both hold as stated. RTD-1 and RTD-12 fire; RTD-11 fires `closed_population`; the new negative fire lands on RTD-13, which already fired `run_tool`; and RTD-8's extra `question_asked` fire is there.
- **Form 4 gate:** the table matches `form4-gate.txt` (6/8, clean-3 and clean-4 failing). The form 4q gate is 6/6.
- **P2 baseline:** "0 of the 9" held under form 3, and the list of 9 positives is correct against the corpus gold.
- **Partial audit:** V = 3, S = 6, N = 0, and every quote passes both mechanical checks. S0 fires nothing on the three V cases under either form.
- **Stage 2 counts:** 946 kept, 513 incidents, 250 document groups (from 257 paths), and the marker-source, kind and hint counts all match the JSONL.
- **Miner reproduction:** the miner re-run from `f828134a^` produced a byte-identical `mined-candidates.jsonl` and identical summary counts. It scanned 4,330 commits against the 4,325 reported, which is only a later branch tip.
- **Fork drafts:** every row of all eight `fork-*.jsonl` files has non-empty `text`, so that held-out filter read real text rather than silently reading nothing.
- **Python tests:** `tests/test_phase1_span_selector_report.py` and `tests/test_phase2_score_dp1.py` pass, 17 tests.

`★ Insight ─────────────────────────────────────`
- Finding 1 happens when one discount is applied to every rule and a second, unstated one to the rule that would otherwise break the conclusion. Checking a claim like "no rule reaches the bar" means running the same arithmetic on every row of the table, not just the one the text names.
- Finding 3 is a filter that matches on text, while the leak is by origin: a sibling sentence from the same commit and incident shares no 8-token run with the held-out case. So leakage filters need to check where a text came from (commit, document), not only its wording.
- Finding 4 shows why a before-and-after claim at a 2-of-3 threshold should quote the baseline row: 1/3 to 3/3 and 0/3 to 3/3 look identical as FAIL, but they are very different evidence.
`─────────────────────────────────────────────────`