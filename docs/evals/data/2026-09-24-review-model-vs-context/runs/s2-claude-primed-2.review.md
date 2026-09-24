`★ Insight ─────────────────────────────────────`
- **An aggregate metric can hide a new cost.** Form 4q's precision check (Q2) counts negative *texts* with any fire. A new wrong fire on a text that already fired leaves that count unchanged, so the metric could not show the new false positive on RTD-13. The per-rule fire count shows it.
- **Carrying rows forward only works if the carried rows are unchanged.** I checked all 882 carried rows in the form 4q file against form 3's file field by field, and none differ. That check is what makes a 4q-vs-3 comparison valid.
- **A text-overlap leakage filter is not an incident filter.** An 8-token match catches copied sentences. It cannot catch a different sentence from the same correction commit, and that commit is exactly what the amended plan says must stay in one fold.
`─────────────────────────────────────────────────`

# Review: S0 forms 4 and 4q, and the Stage 2 mined-pair build

The form 4q numbers reproduce exactly from the committed rows. The problems are in what the documents conclude from those numbers, and in how the Stage 2 filter and its "no rule reaches 50" estimate were built.

## Findings

**1. "No new false-positive text" hides half of the widened `question_asked`'s fires being wrong.**
- (a) The write-up reports Q2 (negative texts with any fire, 5/21 → 5/21) as showing no precision cost. But the widened spec fired 4 times on Score A, and 2 of those fires are wrong.
- (b) Evidence:
  - `form4q-readout.txt`, last block: fires on RTD-1 and RTD-12 (gold), RTD-8 positive (gold is `contradiction`) and RTD-13 negative (gold is none).
  - My re-run of `report_corpus` on `p1s-S0f4q-corpus.jsonl` against `p1s-S0f3-corpus.jsonl`: the `no` bucket's negative fires per text go from 0.25 to **0.50**.
  - The comparison table in `rule-tell-scoring-2026-09-23.md` (§ S0 form 4q) leaves out the fires/text column, which is the only one that shows this.
  - The claim is repeated at `:684` and in handoff item 2 (`:703`).
- (c) Impact: "for `question_asked`, the silence was spec coverage" is presented as costing nothing. Its measured precision on these n=4 fires is 2/4. RTD-13's false fire is on a `file:line` citation, and the corrected texts in this corpus add those routinely. Q2 is a text-level count, so it cannot move when a new fire lands on a text that already fires (the monotone-assertion law in `CLAUDE.md`).

**2. Form 4's `clean-4` failure is blamed on widening, but form 3's unwidened specs already fired both rules on `clean-4`.**
- (a) The doc says widening made the judge apply `scope_instant`'s existing clause, and that widened `count_unit` fires on ordinary counts.
- (b) Evidence: `form3-gate.txt` shows `clean-4 … hit 2/3 fired [[], [], ['count_unit', 'scope_instant']]`. Only `clean-3` is a failure that is new under form 4 (0/3 → 3/3 `count_unit`).
- (c) Impact:
  - For `scope_instant`, the evidence for "naming the shape costs precision" is a single text it already fired on without widening.
  - For `count_unit`, the claim that the law "is not expressible as a sentence-level shape" rests on one new failing text.
  - Both widened specs were dropped on this evidence. The conclusion in the author summary and in § *Across forms 4 and 4q* is stated more strongly than the gate supports.

**3. The Stage 2 "no rule reaches 50" estimate is internally inconsistent.**
- (a) The prereg says "`count_unit` is nearest at 50 raw hints, about 15 after the sample's precision". But `closed_population` has 169 hints, and the same 30–40% factor gives it 51–68, which is over the bar.
- (b) Evidence:
  - `stage2/summary.txt` "any hint": `closed_population 169`, `count_unit 50`, `selector_narrow 47`.
  - The 30–40% figure is a *genuine-pair* rate from 10 rows (3/10 has a Wilson interval of about 11–60%). It is not a measure of whether a hint names the right rule.
  - `count_unit`'s 50 includes repeats of the same ledger lines. My script found 41 distinct 60-character prefixes, 32 incidents, and 5 rows where positive equals twin once digits are ignored. Those are counter updates, not corrections.
- (c) Impact: the go/no-go input to the operator decision rests on a figure that is inconsistent with its own method. The conclusion probably still holds, but only because hint precision is poor, and that has not been measured.

**4. The held-out filter only matches overlapping text, so a candidate from RTD-12/13's own correction commit survived.**
- (a) Candidate `e365a6b3`, in `docs/adrs/2026-09-21-tracker-state-splits-by-recoverability-not-by-shape.md`, incident origin `ddfce065`, was kept.
- (b) Evidence:
  - RTD-12 and RTD-13 name `e365a6b3` as their source, and RTD-14/15/16/18/19/21 share that ADR (`rule-tell-detection.md`).
  - The kept row's positive is *"…split between 15 params-backed and 16 prose-backed trackers…"*, which is RTD-16's subject.
  - `mine_pairs.py:250-259` drops only on a path regex or an 8-token overlap. It has no check by incident or source document against the Score A sources.
- (c) Impact: this is one leaked candidate today, but it contradicts amendment 3 (one incident, one fold) and the status note's "filter against every held-out source". The same hole will let more through as the pool grows.

**5. The planned next step, "spec breadth tested on T", is not in the registration.**
- (a) Stage 4 scores trained arms and C1 on T. C1's second stage is "Haiku, with the form-2 specs". S0 (Sonnet) with widened specs is never scored on T.
- (b) Evidence: `phase1-local-classifier-preregistration.md:97-98` (Stage 4, the C1 line) and handoff item 2 (`rule-tell-scoring…:703`).
- (c) Impact: under the current plan, the test the documents defer to will not happen. T also depends on the blocked Stage 2. Nothing keeps the candidates the author has already read (the 10-row sample) out of T, if T is drawn from the mined pool.

**6. Neither the miner nor the readout script can be reproduced from this commit.**
- (a) Both hardcode another checkout, and the git-log input is not pinned.
- (b) Evidence:
  - `stage2/mine_pairs.py:38` and `form4q-readout.py:4` set `ROOT = /home/marius/work/claude/codescout`. That tree supplies the selector, the controls, the fork drafts and the form 3 rows.
  - The input is `git log experiments`, on a branch that gets rebased, and no tip SHA is recorded. The 97 MB extract is not kept.
  - The held-out set reads `sel.GATE` at import time, so the form-4 gate texts `GATE_F4` are left out (I found 0 overlaps today).
- (c) Impact: "the script regenerates it" (README, prereg) is not true as committed. The 946 rows and the drop counts cannot be re-derived.

**7. The proposed fold-by-document rule already breaks one-incident-one-fold, and the filename merge is real, not theoretical.**
- (b) Evidence:
  - Incident `docs/trackers/issue-clusters.md@0c5bab41b4` (16 candidates) spans `issue-clusters.md` and `IC-14-guard-narrower-than-its-name.md`.
  - Group `bug-tracker.md` merges `docs/issues/bug-tracker.md` with `docs/archive/old-trackers/bug-tracker.md`.
- (c) Impact: the prereg's "could in principle merge" understates it, and the carried-forward fold rule conflicts with amendment 3 on the current data. Low severity.

**8. Small inaccuracies.**
- The author summary says the 92 dropped rows all overlapped held-out texts or came from campaign documents. Two were duplicates (`summary.txt`: `duplicate pair: 2`).
- The prereg says the `closed_population` hint "only matches all / every / none". The regex at `mine_pairs.py:76` also matches "no".

## Checks that came out clean

- **Form 4q Score A:** recomputed with this tree's `report_corpus` for forms 2b, 3 and 4q. Every cell in the scoring doc's comparison table matches, recall 3/17 → 5/17 is correct, and all three files cover 42 of 42 texts with 0 errored.
- **Carry integrity:** 924 rows; 882 carried and identical field by field to form 3's rows; 42 fresh rows, all `question_asked`; no duplicate keys.
- **Q1:** RTD-1 and RTD-12 fire on the audit's sentences. RTD-12's fire lands on the hedge sentence, which the audit also quoted as `quote2`, and which carries the `file:line` citation the tell names. RTD-11 still fires `closed_population`.
- **Gate logs:** form 4 is 6/8 and form 4q is 6/6, the n/a handling is correct, and the `f4-*` fire counts match the doc.
- **Specs:** 4q changes only `question_asked`; the other 21 specs equal form 3's.
- **Stage 2 counts:** 1038 − 92 = 946; 513 incidents; 250 groups; 20 colliding pairs; marker sources 230 / 165; 615 with no hint. The 10-row sample's 3 genuine / 1 weak / 5 mismatched / 1 fragment matches my own reading.
- **Held-out coverage:** every fork file carries a `text` field. The API-route and e2s draft files show only generic phrase overlaps (4 candidates), and `blind-tasks` has none.
- **Archive commit `938e6d0c`:** both files moved to `archive/`, no citation of the old paths remains at HEAD, and the recorded patch-ids for `0fef5562` and `138bdb60` match what `git patch-id --stable` gives.
- **Python tests:** `pytest tests/test_phase1_span_selector_report.py tests/test_phase2_score_dp1.py` gives 17 passed.