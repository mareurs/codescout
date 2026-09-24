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
