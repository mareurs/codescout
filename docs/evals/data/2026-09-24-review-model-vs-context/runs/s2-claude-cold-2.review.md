Review of the rule-tell work since 9c0d2505. The form 4 and 4q numbers hold up against the committed data. The problems are in the Stage 2 status note and in how reproducible the miner's output is.

`★ Insight ─────────────────────────────────────`
- Form 4q changed only one rule (`question_asked`); the other 21 rules' rows were copied from form 3. A Score A built that way can only be trusted if the copied rows are faithful, so I compared them row by row.
- The Stage 2 note's weak points are two numbers. "30–40% genuine" comes from a sample of 10, and "no rule reaches 50" breaks on the note's own hint counts.
`─────────────────────────────────────────────────`

## Findings

1. **(a)** The Stage 2 stopping-rule sentence contradicts the note's own figures. By the note's method, `closed_population` reaches the ≥50 bar, and `count_unit` is not "nearest".
   **(b)** `docs/evals/phase1-local-classifier-preregistration.md:372` says "no rule … `count_unit` is nearest at 50 raw hints, about 15 after the sample's precision". But `stage2/summary.txt` lists `closed_population 169` (any hint), and the same file's line 368 says so too. 169 × 0.30–0.40 = 51–68, which is ≥ 50. I recounted from `mined-candidates.jsonl`: 169 and 50, matching.
   **(c)** The conclusion "the route cannot run on mined pairs alone" is probably still right, but only because `closed_population` hints are likely wrong far more often than pairs are. That rate was never measured: the three `closed_population`-hinted rows in the 10-row sample (47e5fa8a, e99ab5ef, 4f2d20bf) do not look like `closed_population` violations to me. The operator's labelling decision is being made on a derivation that is wrong as written. The note also treats "the pair is genuine" as if it meant "the hinted rule is correct".

2. **(a)** The "About 30–40% genuine" figure is 3 genuine plus 1 weak out of 10 rows, published without an interval. The real uncertainty is several times wider than the stated range.
   **(b)** Prereg `:368`. Wilson 95% interval: 3/10 gives [0.11, 0.60]; 4/10 gives [0.17, 0.69].
   **(c)** "About 15" for `count_unit`, and every downstream count of how many pairs are usable, inherits a precision that could lie anywhere from about 11% to about 69%.

3. **(a)** The miner's input is not pinned, and the README says something false about it. The script does not regenerate the git-log extract: it only reads `gitlog.patch`, and the docstring's command uses the moving `experiments` ref.
   **(b)** The README says "the script regenerates it". In fact `mine_pairs.py` has `mine(HERE / "gitlog.patch")` and no git call. Running the miner's pathspec with `git log <ref> --format=%H … | wc -l` gives HEAD 4339, `experiments` 4348 today, and `f828134a^` 4330. The published 4325 matches only `938e6d0c`, and no SHA is recorded anywhere.
   **(c)** The published counts (946 kept, 513 incidents, 250 docs, 92 dropped) cannot be re-derived. Because `experiments` is rebased after each ship, re-running the documented command will produce different numbers.

4. **(a)** The cross-document collision preview undercounts. `owner.setdefault` only records a pair against the first document that owned a shingle, and the check leaves out the paragraph text, which the held-out filter does include.
   **(b)** `stage2/mine_pairs.py` collision loop and summary line "20". Counting every pair exhaustively (my script) gives 25 document pairs over positive+twin, 83 when paragraphs are included, and 47 at the incident level.
   **(c)** The line "20 document pairs share a shingle" understates how much work the cross-fold filter has. This matters for sizing the folds.

5. **(a)** Deduplication only matches exact (positive, twin) pairs, and incidents are keyed by path. As a result, identical positives land in different document groups, and archive moves split one document into two incidents.
   **(b)** One sentence ("**Mechanism status:** none yet — not checked against the code as of 2026-09-02…") is kept 4 times across 4 IC-* document groups. Seven document groups span both a live and an `archive/` path.
   **(c)** "2 duplicates" and "513 incidents" are both slightly off. Identical training text would leak across folds unless the planned cross-fold filter catches it.

6. **(a)** The form 4q section presents RTD-11's `closed_population` fire as a new observation. It was not re-judged.
   **(b)** `rule-tell-scoring-2026-09-23.md` form 4q section: "RTD-11 is unchanged: it still fires `closed_population`". In `p1s-S0f4q-corpus.jsonl`, that row carries `carried_from: …p1s-S0f3-corpus.jsonl`. Only `question_asked` was judged (42 fresh rows).
   **(c)** Low impact: the sentence claims a re-observation that never happened.

7. **(a)** `mine_pairs.py`, `form4q-readout.py` and `partial-audit-check.py` hardcode `ROOT = /home/marius/work/claude/codescout`. They read the selector, the corpus and the held-out sources from a different working tree than the committed data they sit next to.
   **(b)** I checked those five files in that tree today with `cmp`: all are byte-identical to 898d3ea3, so nothing is wrong yet.
   **(c)** This is a latent problem: once that tree moves, the readouts silently stop describing the committed rows.

## Checks that came out clean

- **Form 4 gate:** 6/8, clean-3 and clean-4 failing, the n/a rows and the fire patterns all match `form4-gate.txt`. The form 4q gate is 6/6 with 0 errored rows.
- **Copied rows in form 4q:** all 882 carried rows match form 3 exactly on `verdict`, `gold` and `text_detectable` (0 mismatches). The 42 fresh rows are all `question_asked`.
- **Score A recomputed:** I ran `report_corpus` in-process on both committed corpora, with no model call. Every figure in the form 3 → 4q table matches, as do recall 3/17 → 5/17, negatives 5/21 → 5/21, and yes-bucket gold-only 2/8 → 1/8, caused by RTD-8.
- **Quotes:** all 19 YES quotes in the form 4q file pass `verify_span` against their texts. Q1 (≥2 of 3) and Q2 (at most +1) are correctly scored against registration `6aa61dff`.
- **Partial audit:** the quote checks reproduce V=3, S=6, N=0. The robustness claim holds: with 3 cases flipped to N, V+S = 6. RTD-4's form 3 fire is on the same sentence as the audit quote. RTD-12's fired sentence contains the audit's quote2 and the `file:line` the audit names.
- **Stage 2 counts:** 946 rows, 937 rewrite + 9 note, marker sources 230/542/165/9, 615 unhinted, and `rule` null on every row all reproduce. No kept row comes from a held-out document path, and all fork-draft files carry `text`, so the held-out filter is not vacuous. A token search for RTD-case correction events found no leaked candidates.
- **Selector wiring:** `--form` rebinds `QUESTION`/`SPECS` globally, and form 4q uses form 3's question plus only the widened `question_asked` spec.
- **Tests:** `tests/test_phase1_span_selector_report.py` and `tests/test_phase2_score_dp1.py` pass (17 passed). I ran them with pytest; no cargo.

`★ Insight ─────────────────────────────────────`
- Findings 3 and 4 fail in the same way: each count was produced by one run of a script, with its input or method left out. The number looks exact, but nobody can re-check it. Recording the extract's SHA in `summary.txt` would fix the first.
- The form 4q result is sound as arithmetic. What limits it is disclosed in the doc: one run per row, and a spec written after reading the cases it was tested on.
`─────────────────────────────────────────────────`