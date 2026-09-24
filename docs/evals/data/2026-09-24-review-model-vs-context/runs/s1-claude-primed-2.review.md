The campaign's arithmetic mostly holds up: most published tables recompute exactly from the committed rows. The defects are in the claims made around those numbers. Two of them, including your recommended next step, affect what gets done next.

## Findings

**1. The reason given for "check the 9 `partial` texts before Stage 2" contradicts the local-route pre-registration, and the silence figure it rests on is off.**
- (a) Your summary says Stage 2 "would label training data with the same excerpt convention". Stage 2 does not use judged excerpts at all. Separately, "S0 fires no rule on 10 of 17" is form 3's figure. S0 is registered at form 2b, where the figure is 13 of 17.
- (b) Evidence:
  - `phase1-local-classifier-preregistration.md:55` says "Labels come from construction, not from a judge": mined correction pairs where the diff supplies the label, plus synthetic pairs.
  - `:66` lists the 21 phase-1A pairs as held out and never used as training input.
  - I recomputed from `p1s-S0b-corpus.jsonl`: form 2b gives yes+partial as gold 3, wrong-rule-only 1, nothing fired 13. Form 3 gives 3 / 4 / 10.
  - Under form 2b, 4 of the 8 `yes` positives are also silent (RTD-9 among them), even though the corpus labels them visible.
- (c) Impact: the `partial` check cannot change "how weeks of labelling are done", so the ordering argument for doing it first falls away. Even if every `partial` excerpt turned out to be unjudgeable, recall on the visible bucket is still 3/8. So one open question does not decide how to read the selector's results.

**2. Stage 4 of the local route (the next planned scoring step) still names the `rtd8` checker and superseded comparison rates.**
- (a) Stage 4's Score B is registered to score RTD-8 with `rtd8`, which fails its gate on the clean channel. It compares against contaminated-channel rates.
- (b) Evidence:
  - `phase1-local-classifier-preregistration.md:99` names "`rtd8`, `rtd9`, `rtd10` and `rtd3r`".
  - `:28` gives the comparison rows as "RTD-8 5/10 … 1b 0/10".
  - `e2s-score-rtd8.txt` shows the clean gate: recorded fixture NO, NO, NO, GATE FAILED.
  - `rtd8c-score.txt` gives the clean rates: arm 0 9/10, 1b 2/10.
  - `:93` and `:48` still say "the 8 texts"; the gate now has 10.
- (c) Impact: run as registered, any local arm's RTD-8 cell cannot be scored. Or it gets compared against a withdrawn baseline. The fix needs a registration amendment before Stage 3/4.

**3. "The claim-bound arms reproduce on the clean channel exactly" is false for RTD-9.**
- (a) RTD-9's arm 1b moved from 3/9 to 4/9.
- (b) Evidence:
  - `fsc-rtd9.txt` reads `1b … 3/9`; `e2s-score-rtd9.txt` reads `1b … 4/9`.
  - The claim is at `rule-tell-scoring-2026-09-23.md:424`: "the contamination changed no comparison".
  - The API re-score's registered prediction rests on it: `rule-injection-timing-preregistration.md:387`, "since their fork-route rows reproduced exactly".
- (c) Impact: the published "denominator" (contamination changed nothing) is wrong for one of three rules. No verdict changes, because that 1b is spillover.

**4. The "12 of 12 runs" figure behind the form-2b ruling is wrong.**
- (a) H0-clean fired `member_vs_population` on clean-2 in only 1 of 3 runs.
- (b) Evidence:
  - `p1s-H0clean-gate.txt` clean-2 fired `[['member_vs_population','question_asked'], ['question_asked'], ['d_history']]`.
  - `p1s-gate2.txt` has 3/3 and `p1s-S0-gate.txt` has 3/3, so the logged total is 7 of 9. Sonnet never ran on the dirty channel.
  - Claimed at `rule-tell-scoring-2026-09-23.md:341` and `phase1-local-classifier-preregistration.md:172`.
- (c) Impact: the stated basis for narrowing the spec overstates how consistently the spec, rather than the model, caused the fire.

**5. The L0 "ranking carries signal" claim, the stated lead for Stage 3, does not hold as written.**
- (a) The claim is that gold ranks 1st or 2nd on 4 of 5 texts and scores above the clean maximum. On `cannot` the margin is exactly 0.00, and `sessionid`'s "rank 2" is a four-way tie.
- (b) Evidence from recomputing `l0-gate.jsonl`:
  - `cannot_happen` is 0.600599 on both `cannot` and `clean-1`.
  - `d_sessionid` is 0.5, tied with `run_tool`, `open_artifact` and `d_visibility`.
  - The 220 probabilities take only 41 distinct values.
  - Claim at `rule-tell-scoring-2026-09-23.md:561`, repeated in the handoff at `:617`.
- (c) Impact: only 2 of 5 texts (`semicolon`, `member`) show an unambiguous top rank *and* separation from the clean texts. The zero-shot evidence that per-rule calibration is the lead is weaker than stated.

**6. Several results have no backing file, although the README says the directory is the evidence for every reported result.**
- (a) Three gaps:
  - `score-rtd910.txt` contains no scores. It is four runs that crashed on the API usage cap, one of them after its gate had passed.
  - The API-judge "arm 0 4/10 (RTD-9), 5/10 (RTD-10)" (`rule-tell-scoring-2026-09-23.md:161`) and Jev's "`none` on 10/10 drafts" (`:198`) have no rows in the directory.
  - `phase2-score-dp1.py` never prints its judge config dir, so the "clean channel" provenance of `api-score-*`, `e2s-score-*` and `rtd8c-score.txt` is not recorded in them.
- (b) Evidence: `grep -c "violation rate" score-rtd910.txt` returns 0, and the file is all `BadRequestError: … usage limits` tracebacks. `README.md:35` labels it "the original scores".
- (c) Impact: the "judge channel moves rates" claim cannot be checked, and the headline clean re-score depends on an environment variable nobody logged.

**7. The "grep-first" description of the RTD-3 forks is wrong.**
- (a) The 3 / 6 / 3 counts are forks whose first tool was not the doc write, but most of those first tools are `Agent` or `run_command`, not `grep`.
- (b) Evidence from recomputing `fork-rtd3r.jsonl` first tools:
  - arm 0: grep, grep, Agent;
  - arm 2: Agent ×5, grep ×1;
  - 3-1b: run_command ×2, Agent, and no grep at all.
  - The old run's "6 grep-first" is 4 grep plus symbols plus Agent.
  - Claims at `rule-tell-scoring-2026-09-23.md:225`, `:227` and `:229`.
- (c) Impact: the arithmetic bound still holds. But the Codex-narrowed scope note describes the wrong behaviour. A fork that hands the work to a subagent is a different "later behaviour" from one waiting on a grep.

**8. The stripped-arm prediction "s0 is not below arm 0" is marked "Holds" against the wrong comparison number.**
- (a) Under `rtd8c`, s0 is 7/8 = 0.875 and fork arm 0 is 9/10. The doc falls back on "the registered 8/10", which is the API route's arm 0.
- (b) Evidence: `rule-injection-timing-preregistration.md:240` says the predictions are "read against fork arm 0 rather than the API route's 8/10". The verdict is at `rule-tell-scoring-2026-09-23.md:456`.
- (c) Impact: small. The honest reading is "marginally below, within noise", not "holds".

**9. A single-run pick is cited as Jev's answer.**
- (a) "Jev ranked the correct rule first (`contradiction`, 0.40)" is run 0 only. Run 1's top pick was `none`.
- (b) Evidence: `phase1-jev.jsonl`, RTD-8 positive top picks are `contradiction` 0.40, `none` 0.34, `contradiction` 0.35. Claim at `rule-tell-scoring-2026-09-23.md:135`, against the doc's own rule at `:277`, "never read one call as the answer".
- (c) Impact: low, but it overstates phase-1 support for the one case both phases use.

## Checks that came out clean
- **Score A**, both forms: 924 rows each, 22 rows per text, 0 errors. The bucket table, 3/17 recall, 2/21 → 5/21 negative fires, the per-case fires, family recall 5/17 and Wilson [0.06, 0.41] all recompute.
- **Phase 0**: 2,850 rows at n = 10. The positive fire rates, all diagonal near-miss and full-shape cells, and the off-diagonal rates (0.33 / 0.30 / 0.22 / 0.08 / 0.07) all match.
- **Phase 1A**: Jev 12/51, 27/51, 30/63, mean P(none) 0.27 / 0.24, top pick stable on 33 of 42 states, and 0 identical distributions. Haiku 12/85, 30/85, 1/105.
- **Phase 2**: every arm in the API DP1 table, the RTD-9/10 subscription tables, the fork-route and stripped table, the RTD-3 tables, `rtd3r` main and exploratory, the `rtd8c` table, and the API clean re-score (margins 1.00 / 0.90, 0.60 / 0.70, 0.90 / 1.00) match their logs. The Score B RTD-8, RTD-10 and RTD-3 cells, and the RTD-10 5/10 and 0.30 fallback, are correct. Stripped API arms: 17 of 30 are error rows.
- **`e2s` injections**: which drafts bound which rules (DP1 runs 1 and 3; `contradiction` only in run 1; 2 drafts with no injection and 6 with other rules only; RTD-3 has 6 drafts with no injection and `monotone_absence` only in runs 0 and 7).
- **Gate logs**: H0, H0-clean, S0, S0 2b, form 3 and L0 gate and span-gate tables match, including form 3's single stray fire on clean-4.
- **Code**:
  - `report_corpus` refuses incomplete sweeps.
  - `Counter(RULES.keys())` is correct.
  - `QUESTION_FORMS["3"]` differs from 2b only in the one sentence it is meant to drop.
  - L0's `sel.judge_rule` swap does reach `sweep` and `span_gate`, and the row order is deterministic with `pool=1`.
  - The Score B ship-rule reading ("n/a" for RTD-9 and RTD-10) matches `rule-injection-timing-preregistration.md:312`.
- **README**: every file in the directory is listed.

`★ Insight ─────────────────────────────────────`
- The data came out cleaner than the prose. Every table computed by a script matched, and the defects sit in hand-written summary sentences like "reproduces exactly", "12/12" and "grep-first". A statement written about a table is not re-checked the way the table is.
- Two findings (3 and 4) are claims that later registrations were built on. When a registration rests on an earlier summary sentence, an error in that sentence carries into the next experiment's design.
`─────────────────────────────────────────────────`