# Review: model or context? Why two Codex reviews caught what the authoring session missed — registration and results

**Valid:** dated 2026-09-24

**Status:** registered 2026-09-24, before any review or checker call. Results are appended below the registration, never edited into it.

## Question

On 2026-09-24, two Codex reviews of the rule-tell campaign found 11 defects that the authoring Claude session (Opus 5.5, session `571eb3d6`) had committed without noticing. All 11 were later re-derived and confirmed: `docs/research/2026-09-24-codex-rule-tell-review.md` and `docs/research/2026-09-24-codex-rule-tell-followup-review.md`. The operator's question: **was that the model (Codex/GPT against Claude), or the context (a cold reviewer holding only the committed artifacts, against one carrying the author's framing)?**

## Design — a 2 × 2 on each of two stimuli

**Factor MODEL:**

- **Claude:** `claude-opus-5-5`, the author's own model, via `claude -p` on the `~/.claude` subscription.
- **Codex:** `gpt-5.6-sol` at `model_reasoning_effort = "high"`, the configuration the real reviews ran with, via `codex exec` on its ChatGPT subscription (`auth_mode chatgpt`, no API key present).

Both reviewers get the **same clean channel**: no user-level instructions (no `~/.claude/CLAUDE.md`, no `~/.codex/AGENTS.md`), no plugins, no hooks, no MCP servers, shell and file tools only, and every API-key variable stripped from the child environment. Each CLI loads the repo's own instruction file natively: Claude auto-loads `CLAUDE.md`; Codex loads `AGENTS.md`, a pointer telling it to read `CLAUDE.md`. **Disclosed asymmetry:** the real Codex reviews ran with the operator's global `AGENTS.md` and the codescout MCP server, so the Codex cells are not a replay of them.

**Factor CONTEXT:**

- **COLD:** a fresh detached worktree at the stimulus commit, plus a neutral brief naming only the scope.
- **PRIMED:** the same worktree and brief, plus the author's own summary of the work, as reported to the operator before the review (`primed-s1.md`, `primed-s2.md`). These are excerpts of the author's messages, verbatim except that persona lines, an unrelated disk-usage aside and the explanatory "Insight" boxes are removed. This is the hypothesised mechanism: "read the summary, not the data".

**Stimuli:**

| | commit | scope (the brief's wording is in `run_reviews.py`) | known defects |
|---|---|---|---|
| s1 | `a8835d06` | the rule-tell campaign's scoring doc, both pre-registrations, four scripts, the data directory | 8 |
| s2 | `898d3ea3` | the form 4 / 4q results and the Stage 2 miner, committed since `9c0d2505` | 3 |

**No arm sees an answer.** Each worktree is at the stimulus commit. s1 contains no Codex rule-tell review; s2 contains review 1 (a prior review, not review 2's answers) and neither miner bug file. `git grep` at `898d3ea3` for the answer strings (`30/946`, `star edge`, `25 pairs`) returns nothing. The primed texts predate each review.

**n = 3 per cell, 24 runs in all**, in seeded random order (`random.Random(20260924)`), 4 in parallel, 40 minutes each at most. **Why not 10:** each run is a full agentic review of a large scope on Opus 5.5 or GPT-5.6 at high effort, on subscriptions that hit their limits earlier today. So results are reported as counts with their denominators, and **no cell comparison is called significant**. An errored or timed-out run is reported, never scored.

**The known-defect set** (defined in `check_reviews.py`) is the 11 defects the real reviews found. s1: D1 C1 chosen on T; D2 negatives over-claimed / no incident fold split; D3 Score A accepts missing texts; D4 phase-2 scorer keeps no per-row votes / no clean-channel refusal; D5 JevK5 `cannot_happen` ties clean-1; O1 the "four of five" conjunction; O2 7/8 against 9/10; O3 "upper bound". s2: Ma the post-correction `paragraph` context; Mb 20 pairs against 25; Mc the "no rule reaches 50" claim.

**Checker, gated.** One question per defect, judged by Sonnet 5 on the clean judge channel, majority of 3. **Gate, run first; a failure stops scoring.** For every defect, the real Codex review must be judged YES, and the same review with that defect's paragraph(s) removed must be judged NO. The removal is the output mutation that shows the checker can go red. 22 fixtures in all.

**Precision.** Findings other than the 11 are counted per cell. A seeded sample of 10 of them is checked against the repo by the experimenter, and reported as valid, invalid or unverifiable.

**Binding.** The review text is kept for every run. The experimenter reads every miss for the defects the real review found and reports any case where the checker's verdict contradicts the text.

## Measures

- **Recall per cell** = defects found / (reviews × known defects), per stimulus and pooled.
- **Context effect** = mean over models of (COLD − PRIMED) pooled recall.
- **Model effect** = mean over contexts of (Codex − Claude) pooled recall.
- **Reference point, not a cell:** the authoring session itself found 0 of 11 before the reviews.

## Predictions

- **P1:** the model effect is below 0.10.
- **P2:** the context effect is at least 0.20. PRIMED misses more, most of all Mc, the claim the primed s2 text repeats.
- **P3:** COLD-Claude pooled recall is at least 0.50.

## Reading, fixed now

- **"Context":** context effect ≥ 0.20 and model effect < 0.10.
- **"Model":** model effect ≥ 0.20 and context effect < 0.10.
- **"Both":** both ≥ 0.20.
- **"The review role":** both effects below 0.10 and every cell's pooled recall ≥ 0.50. Then even a primed reviewer of either model catches most of what the author missed, and **what differed was being asked to review rather than to build**, which neither factor varies.
- **Otherwise: not settled at this n**, reported per cell and per defect.

## Limits, stated now

- **Tailored set:** the known-defect set was defined by what Codex found, so recall is measured on Codex's own findings. This favours the Codex cells, and a Codex cell reaching 1.0 is partly by construction. Defects no one has found are invisible to it.
- **Small n:** 3 reviews per cell.
- **Stimulus reuse:** both stimuli were chosen after the fact.
- **PRIMED approximates the author's state; it is not the author.** The author was building, not reviewing.
- **The optional WARM arm (a true fork of the authoring session) is dropped:** it cannot be matched for Codex, and a cross-profile resume of a 1M-token session is not a clean channel.

## Results — appended 2026-09-24, after every run and score
> **Correction, 2026-09-24, from the operator, verified: the Codex cells did not run the real reviews' model.** The registration says `gpt-5.6-sol` at `high` is "the configuration the real reviews ran with". It is only the default in `~/.codex/config.toml`. The Codex session that wrote both real reviews, `~/.codex/sessions/2026/09/20/rollout-2026-09-20T12-14-32-01a0be18-….jsonl`, records **`gpt-6-astra` at `medium`**: 94 model fields and 84 effort fields, with no other value. So the Codex cells measure a **different GPT model and effort** from the reviewer whose catches motivated this experiment. The Claude cells, the context comparison and the per-model split of what gets found stand as measured. What the Codex cells cannot show is how the real reviewer compares, or whether its findings came from its model. A matched `gpt-6-astra`/`medium` arm would need a new registration.

**Runs:** 24 of 24 completed with exit 0. Every run reported its pinned model (`claude-opus-5-5`, `gpt-5.6-sol`), left its checkout clean, and ran 4–14 minutes. **Checker gate: 22/22**, and every fixture's 3 votes were unanimous. Over the 132 scored (review, defect) pairs, 131 of the vote triples were unanimous.

**Recall** (defects found / known defects × 3 reviews):

| cell | s1 (of 24) | s2 (of 9) | pooled (of 33) |
|---|---|---|---|
| Claude, COLD | 11 | 4 | 15 = 0.45 |
| Claude, PRIMED | 10 | 3 | 13 = 0.39 |
| Codex, COLD | 7 | 7 | 14 = 0.42 |
| Codex, PRIMED | 8 | 7 | 15 = 0.45 |

**Per defect** (found in how many of 3 reviews; Claude cold / primed · Codex cold / primed):

| defect | Claude | Codex |
|---|---|---|
| D1 C1 chosen on T | 0 / 0 | 0 / 1 |
| D2 negatives over-claimed / no incident split | 0 / 0 | 1 / 3 |
| D3 Score A accepts missing texts | 0 / 0 | 3 / 1 |
| D4 phase-2 scorer, no per-row votes | 3 / 2 | 3 / 3 |
| D5 `cannot_happen` tie | 3 / 3 | 0 / 0 |
| O1 "four of five" conjunction | 3 / 3 | 0 / 0 |
| O2 7/8 against 9/10 | 2 / 2 | 0 / 0 |
| O3 "upper bound" | 0 / 0 (see binding) | 0 / 0 |
| Ma post-correction context | 0 / 0 | 1 / 1 |
| Mb 20 pairs against 25 | 1 / 0 | 3 / 3 |
| Mc "no rule reaches 50" | 3 / 3 | 3 / 3 |

**Effects:**

- **Context effect = 0.015**; per stimulus, s1 0.00 and s2 +0.06.
- **Model effect = 0.015**, but in opposite directions per stimulus: s1 −0.13 (Claude ahead), s2 +0.39 (Codex ahead).

**Against the predictions:**

- **P1 (model effect < 0.10): held pooled**, but only because the two stimuli cancel. It is not a null.
- **P2 (context effect ≥ 0.20): failed.** Priming changed nothing measurable. **Mc, the claim the primed s2 text repeats, was found by all 12 s2 reviews**, primed ones included.
- **P3 (COLD-Claude ≥ 0.50): failed**, at 0.45.

**Registered reading: "not settled at this n."** Both effects are below 0.10, but no cell reaches the 0.50 that the "review role" reading requires.

**What the data do show, beyond the registered reading. Descriptive only, n = 3 per cell.**

1. **The model decides which defects are found, not how many.** Claude reliably finds the **numeric claims recounted against the data** (D5, O1, O2: 16 of 18 Claude reviews on s1, 0 of 18 Codex). Codex reliably finds the **code and protocol holes** (D3, D2, Mb, Ma: Codex 16 of 24 review-defect pairs, Claude 1 of 24). Pooled, the two cancel.
2. **So mixing models is what adds coverage.** Averaged over all pairings, **a Claude review plus a Codex review covers 0.62–0.78 of the known set, against 0.42–0.72 for two reviews by the same model**: s1 cold 0.62 against 0.42; s1 primed 0.67 against 0.46; s2 cold 0.78 against 0.72; s2 primed 0.78 against 0.61. **Every cell's reviews combined** reach 6 of 11 defects for Claude, 7 of 11 for Codex, and **10 of 11 together**.
3. **The author's own framing did not blind a reviewer.** PRIMED is within one review of COLD in all 11 Claude defect rows and in 9 of the 11 Codex rows. The two exceptions, Codex on D2 (1 → 3) and D3 (3 → 1), move in opposite directions.
4. **The review role is the largest difference observed, though it is not a registered cell.** The authoring session, Opus 5.5, found **0 of 11**. The PRIMED Claude reviewer, the same model given the author's own summary, found **13 of 33**. Neither factor tested accounts for that gap. What differs is being asked to review rather than to build.

**Binding: every miss was read against the review text.**

- **O3 is a checker false negative, by definition, in 5 reviews:** Claude cold-0, cold-1, primed-0, primed-1, and Codex cold-2. Each reports that the 5/17 "upper bound" is not one, with a different valid reason (a regrouping of the same outputs reaches 7/17). The registered definition required Codex's reason (a re-run with changed specs), so they score NO. **With O3 counted by substance:** Claude s1 13 and 12 of 24, Codex s1 8 and 8. This widens the s1 gap in Claude's favour, and no reading changes.
- **Ma, near-miss:** one Claude s2 review observed "the new-side paragraph no longer contains the removed hedge" in a held-out-filter finding, but did not report the label leak. Scored NO, and kept NO.
- **D3, genuine Claude miss:** two Claude reviews explicitly cleared `report_corpus` as working.

**Precision: a seeded sample of 10 of the 206 numbered findings** (`precision-sample.json`). 2 are known defects (O2, Mb). **8 are other findings that hold up when checked**, including:

- a document group that merges two different `bug-tracker.md` files (verified);
- "RTD-11 still fires" in the form-4q write-up when that row was carried, not re-judged;
- the miner not regenerating its own input, as its README says it does;
- a fold rule that moves incidents where the amendment requires whole documents to move.

**0 are invalid.** Claude produces more findings per review (s1 74 against Codex 51; s2 46 against 35), so neither model's extra output is noise in this sample.

**Limits carried from the registration, plus two found in running:**

- **The known set is Codex's own findings, and Claude still led on s1.** So the s1 direction runs *against* the tailoring's bias.
- **The O3 definition bundled a reason with the defect.** One concept per rubric would have avoided it.
- n = 3 per cell; two stimuli, both chosen after the fact. The Codex cells ran without the global `AGENTS.md` and the codescout MCP server that the real reviews had.
