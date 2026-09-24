`★ Insight ─────────────────────────────────────`
- The most useful check was to rebuild each table from the per-row `.jsonl` files instead of the `.txt` summaries. Most of the headline numbers reproduce exactly. The defects are in how the prose extends them: "four of five", "reproduces exactly", "the re-score is complete".
- Several defects are timing problems. Whether a score ran on the clean or the contaminated channel depends on the commit it came from, so `git log --date=iso` on the registration SHAs was the discriminator (finding 1).
- `phase1-local-l0.py`'s `noul` log has no text id. Pairing rows with gate texts relies on `pool=1` keeping the order, and asserting the rule order repeats every 22 rows was what made the L0 check safe (finding 2).
`─────────────────────────────────────────────────`

## Findings

**1. RTD-3's "ships" verdict and the claim that "the owed clean-channel re-score is complete" both rest on arms that were never re-scored on the clean channel.**
- (a) The ship rule's second clause for RTD-3 (arm 2 − 3-1b = 0.80) exists only as a contaminated-channel score. The same applies to RTD-9's and RTD-10's fork-route s0 arms. The doc still says every comparison the campaign rests on has been scored clean.
- (b) Evidence:
  - `rule-tell-scoring-2026-09-23.md:599` says "Every phase-2 comparison that the campaign rests on has now been scored on the clean channel: RTD-3 … on the fork route earlier."
  - The only clean `rtd3r` run is `data/…/e2s-score-rtd3r.txt`, which scores arms `0`, `3-1b` and `e2s` only.
  - Arm 2 (8/10) exists only in `fsc-rtd3r-main.txt`.
  - `git log` shows the contaminated result was scored before the contamination was found: RTD-3's result commit `b12f2249` is dated 2026-09-24 08:10, and `938799d0`, which discovered it, is 10:07.
  - Likewise, s0 for RTD-9 and RTD-10 appears only in `fsc-rtd9.txt` and `fsc-rtd10.txt`.
- (c) Impact:
  - The campaign's own evidence shows a checker can flip with the channel: `rtd8` went from 3/3 YES to 0/3 on its own positive (`e2s-score-rtd8.txt`).
  - So "the content, not the interruption, stops RTD-3" is unverified on the clean channel.
  - The stripped-arm claim "s0 is not below arm 0 for all three rules" is also still contaminated for RTD-9 and RTD-10.
  - The four-decision-point "binding works" story that the author summary relies on has one leg still unconfirmed.

**2. The L0 (JevK5) "ranking carries signal on 4 of 5 violation texts" claim is false against `l0-gate.jsonl`. It holds on 3 of 5.**
- (a) Claimed at `scoring:561`, and in the handoff at `:618`, which calls it "the lead for Stage 3".
- (b) Evidence: I rebuilt the ranks from `l0-gate.jsonl`, 220 rows, 10 texts × 22 rules, with the rule order asserted.
  - **`cannot`:** gold `cannot_happen` = 0.600599, exactly equal to its value on clean-1 (and semicolon). It is not "above the clean max"; the doc's own table prints +0.00.
  - **`sessionid`:** `d_sessionid` = exactly 0.5, tied 2nd–5th with `run_tool`, `open_artifact` and `d_visibility`, behind `lines_read` at 0.58. Its gate PASS rides on a tie at exactly the threshold (`p < 0.5` → NO).
  - The probabilities are coarsely quantised: 0.5, 0.479613 and 0.439109 recur across many rules. So "rank 1st or 2nd" is largely tie-breaking.
- (c) Impact:
  - A per-rule threshold cannot separate `cannot` from clean-1 at all.
  - The zero-shot evidence for "per-rule calibration is the lead" is 3 texts, one of them a tie at the threshold. That is weaker than the handoff presents to a 1.5–3-week Stage 2/3 investment.

**3. The recommended next step (a check of the 9 `partial` texts) uses silence counts from the wrong form. On the adopted form, the `partial` question cannot explain about a third of the silence.**
- (a) The summary and `scoring:616` say that on 10 of 17 positives S0 fires nothing, and that "most of that silence is the 9 partial texts, 0 of 9 under both forms". But:
  - 10 is the form-3 count. S0 stays at form 2b, where it is **13/17**.
  - "0/9" is recall, not silence: under form 3, RTD-4 and RTD-11 (both `partial`) did fire.
- (b) Evidence: I classified each positive in `p1s-S0b-corpus.jsonl` and `p1s-S0f3-corpus.jsonl`.
  - **Form 2b:** gold 3, wrong rule 1, silent 13. Of the silent 13, 9 are `partial` and **4 are `yes`**: RTD-9, RTD-15, RTD-17 and RTD-18.
  - **Form 3:** silent 10, of which 7 are `partial` and 3 `yes`.
  - `rule-tell-detection.md` labels RTD-9 a **HARD GATE** ("tell wholly inside the published text") and RTD-15 a "secondary hard gate". S0 was silent on both.
- (c) Impact:
  - Even a fully positive result on the `partial` check leaves `yes`-bucket recall at 3/8, with 4 of 8 silent.
  - So the claim that it "decides how to read [the selector's] results" is overstated. The selector is also silent where the corpus says the violation is plainly visible.

**4. The reason given for running the `partial` check before Stage 2 misstates the registration.**
- (a) The summary says Stage 2 "would label training data with the same excerpt convention".
- (b) Evidence from `phase1-local-classifier-preregistration.md` § *Stage 2*:
  - Labels come "from construction": mined correction diffs at sentence level, plus synthetic contrastive pairs.
  - A 10% per-source audit is run against the spec, and a source above 20% disagreement is dropped.
  - The 21 phase-1A pairs, whose `[…]`-joined excerpts are the `partial` texts in question, are held out and never used as training input.
- (c) Impact: the claimed dependency, where an unjudgeable `partial` excerpt means unjudgeable training labels, is not in the registered design. The ordering argument needs restating, or the audit step already covers it.

**5. The local-route pre-registration's Stage 4 Score B is still wired to the failed checker and to superseded baselines.**
- (a) Evidence:
  - Stage 4 names the checkers "`rtd8`, `rtd9`, `rtd10` and `rtd3r`" (`phase1-local-classifier-preregistration.md:99`).
  - Its comparison rows are "DP1 arm 0 (RTD-8 5/10 …), arm 1b for RTD-8 (0/10)" (`:28`).
- (b) `rtd8` fails its gate on the clean channel (`e2s-score-rtd8.txt`: recorded → NO, NO, NO, "replays NOT scored"). The clean `rtd8c` rates are 9/10 and 2/10 (`rtd8c-score.txt`). No amendment substitutes `rtd8c`.
- (c) Impact: run as registered, any local arm's RTD-8 cell halts at the gate. Run with `rtd8c` instead, it would be a post-hoc substitution, and the listed baselines are the contaminated ones.

**6. "The claim-bound arms reproduce on the clean channel exactly for RTD-9, RTD-10 and RTD-3" is false for RTD-9.**
- (a) Location: `scoring:424`.
- (b) Evidence: `e2s-score-rtd9.txt` shows 1b at 4/9, against 3/9 in `fsc-rtd9.txt`. The doc's own table prints "4/9 [3/9]".
- (c) Impact: the "published denominator" claim ("the contamination changed no comparison") is overstated for RTD-9. Its `e2s − 1b` gap is n/a, so the RTD-9 verdict itself is unaffected.

**7. The family-recall "upper bound" is not an upper bound.**
- (a) Location: `scoring:528`, which says "no fairer grouping can do better on these rows" at 5/17.
- (b) Evidence: on the form-3 rows, 7 of 17 positives fire *something*. RTD-20 (`selector_narrow` for `lines_read`) and RTD-11 (`closed_population` for `question_asked`) sit outside the chosen family. Any coarser grouping reaches 6/17 or 7/17.
- (c) Impact: the stated bound is wrong, but the conclusion survives because 7/17 = 0.41 is still below 0.5. The bound should be stated as 7/17.

**8. Some reported numbers have no backing in the committed data, and the README misdescribes one file.**
- (a) `score-rtd910.txt`, listed in the README as holding "the original scores", contains one passing gate and then four `anthropic.BadRequestError … usage limits` tracebacks, with `exit=1` and no rates.
- (b) The claims "Arm 0 measured 4/10 (RTD-9) and 5/10 (RTD-10) under the API judge" (`scoring:161`, and pre-registration lines 187 and 222) have no committed rows. The same is true of:
  - the Jev `noul` gate table;
  - "Jev picked `none` on all 10 (p 0.23–0.46)".
- (c) Impact: the README's promise that "each file's result is the one the scoring doc reports" fails for these. The judge-channel-shift argument at `:161` cannot be checked from the data folder.

**9. The clean-channel claim for every `phase2-score-dp1.py` result cannot be verified from its logs, and the script's default is the contaminated channel.**
- (a) Evidence:
  - `scripts/phase2-score-dp1.py:212-213` defaults `JUDGE_CONFIG_DIR` to `~/.claude-kat` and runs no dirtiness check.
  - Unlike the selector (`phase1-span-selector.py:466`), it prints no `judge: … config …` line.
  - So the logs for `api-score-*.txt`, `e2s-score-*.txt` and `rtd8c-score.txt` carry no record of which channel ran.
- (b) Impact:
  - The claims that RTD-8, RTD-9 and RTD-10 ship on the clean channel, and that `rtd8c` passes clean, rest on the env var having been set. Indirect support exists: `rtd8` fails there.
  - Any re-run without the env var silently reproduces the contaminated channel. The doc (`:622`) documents this trap instead of fixing it.

**10. The handoff points at a path that no longer exists (minor).**
- `scoring:624` says "See `scratchpad/score-b-s0.sh`", but the scratchpad is gone. The script is now `docs/evals/data/2026-09-24-rule-tell/score-b-s0.sh`.

## Checks that came out clean

- **Phase 0**, rebuilt from `scored-reasoned-n10.jsonl` (2,850 rows, 285 tasks × 10) against the labels in `rule-tell-controls.md`:
  - positive fire rates: 1.00, 0.90, 0.80, 0.00;
  - diagonal: RTD-10 0.83 (9/10), near-miss 0.10; RTD-9 0.60 (4/6) and 0.25 (1/4); RTD-8 0.46 (4/8) and 0.20 (0/2); RTD-3 0.39 (5/10);
  - off-diagonal rates 0.33, 0.30, 0.22, 0.08 and 0.07;
  - bare native form: 0/8 near-miss fires.
- **Phase 1A:** Jev top-1 12/51, top-3 27/51, `none` 30/63, stable top pick 33/42, distributions differing on 42/42, mean P(none) 0.27 against 0.24, plus the RTD-10, RTD-15 and RTD-8 probabilities. Haiku 12/85, 30/85, 1/105.
- **Phase-1 gate tables** (form 1, form 2, H0-clean, S0, S0 form 2b, form 3, span gates) match their `.txt` logs.
- **Score A (form 2b and form 3):** buckets, fires/text, negatives 2/21 and 5/21, localisation 3/3, Wilson [0.06, 0.41], the per-case hits and misses, and the table of positives by outcome (gold, wrong rule, silent).
- **Phase-2 API route:** RTD-8 (8/10, 8/10, 0/9, 9/10, 0/10); RTD-9 and RTD-10 on the subscription channel; the clean re-score tables and ship arithmetic (1.00 and 0.90; 0.60 and 0.70; 0.90 and 1.00). Stripped arms: 17 of 30 error rows.
- **Fork route:**
  - `rtd8`, `rtd9` and `rtd10` stripped-arm tables, `rtd8c` (9/10, 2/10, 7/8, 7/9, 1/10, 9/9);
  - RTD-3 old (3/3, 0/1, 0/5), `rtd3r` main and exploratory tables;
  - grep-first counts 3, 6 and 3 (6 in the old run), and the arithmetic bounds 5/7 and 2/4;
  - the observability counts recomputed from `tools` and `text` match every "not-observable" column.
- **Score B:** e2s rates and margins (−0.10, 0.80, 0.27, 0.51, 0.40), the RTD-10 "5/10 if counted" figure, and the `e2s-dp1.jsonl` claim-binding account (runs 1 and 3, `contradiction` on a different sentence, 2 empty and 6 other-claim runs).
- **L0:** the gate outcomes, the p range 0.09–0.86, and "`cannot_happen` at 0.44–0.60, fires on 9 of 10".
- **Data folder:** 76 data files plus the README. No hits for key or email patterns, or for private global-`CLAUDE.md` markers. Every file is covered by the README's names or globs.