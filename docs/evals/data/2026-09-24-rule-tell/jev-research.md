# TypeSafe Jev (System One) for phase-1 rule + claim-span selection: research report

Date: 2026-09-24. Scope: web and docs research only. The Jev API was **not** called and no key was read.
Every claim about Jev cites a URL. `[INFERRED]` marks my own reasoning, not documentation.
"Not documented" means I read the relevant pages and found nothing on the point.

In-repo context read first: tracker `0517d18ca272e132` (typesafe-jev-report-feedback),
`docs/research/2026-09-18-local-semantic-evaluator-review.md`, `docs/trackers/local-semantic-evaluator-design.md`,
`docs/evals/rule-tell-scoring-2026-09-23.md` (the Jev `noul` section, Phase 1A, and the phase-1 handoff),
and `scripts/phase1-rule-selection.py::jev_select` (lines 141-155).

---

## TL;DR

1. **Jev has exactly three question types: `choice`, `score` and `noul`.** It has no extraction, span, free-text,
   rationale or multi-label type, and it cannot generate text. It can still return a span if you frame the span as a
   **choice among spans that code has already segmented**, e.g. line IDs. TypeSafe documents this exact pattern:
   the "line-by-line search" and "pre-parsed value extraction" cookbooks.
2. **Truncation is an unlikely cause of "none on 10/10".** The documented limits are 32k tokens for the state plus
   the longest question, and 64k tokens per request. A 5-30 KB draft is roughly 1.5k-8k tokens `[INFERRED, ~4
   chars/token]`. What Jev does *past* the limit (error or truncation) is **not documented**.
3. **Our question shape goes against TypeSafe's own guidance in several ways:**
   - One compound, 23-way "which rule is violated" question with a `none` option. The docs call for atomic
     questions.
   - A `choice` is *relative*. TypeSafe's own 182-option skill cookbook drops the `none` option and gates with
     separate `noul`s instead.
   - The docs warn about "context rot", large states with irrelevant detail, literal reading, and indirection.
4. **Best documented configuration for our use case:** per draft, fan out two questions per rule:
   - `noul` "does this draft make <rule's claim-shape>";
   - `choice` over line IDs "which line makes it".

   Both run in one request over a line-numbered state. Code copies the chosen line verbatim as the claim. This is
   built only from documented fields.
5. **Honest fit:** Jev can *localise* a claim to a code-segmented unit, cheaply (about $0.001/draft) and quickly
   (vendor figure: 70-500 ms). It **cannot write or trim** a claim. Its documented weaknesses (literal reading,
   indirection, counting, context rot) overlap heavily with what our rules police. On our own `noul` gate it passed
   3/5 prompts, with near-miss failures. Recommendation: use Jev at most as a **candidate localiser/prefilter in
   front of the Haiku per-rule judge**, not as a replacement. Measure that against Haiku-only within one route.

---

## 1. Question types and exact schemas

Endpoint `POST https://api.typesafe.ai/v1/systemone`, with headers `Authorization: Bearer`, `Content-Type:
application/json`. Top-level fields ([API ref](https://docs.typesafe.ai/api.md)):

| field | type | notes |
|---|---|---|
| `state` | string \| object \| array | required |
| `model` | string | required; `"jev-latest"` recommended |
| `questions` | map<string, Question> | required. The key "is not sent to the underlying model and is not used in inference" |

No other request parameters are documented: no temperature, seed, top_k, examples field or return options
([API ref](https://docs.typesafe.ai/api.md);
[Python question types](https://docs.typesafe.ai/sdk/python/api/types/questions.md)).

Question types (all from the [API ref](https://docs.typesafe.ai/api.md) unless noted):

- **`noul`** (yes/no):
  - `type:"noul"`, `instructions` (string|object|array), optional `criteria: {true: EntryType, false: EntryType}`.
  - Answer: `{type, noul}`, where `noul` ∈ [0,1] is P(yes). It has **no `confidence`**
    ([noul](https://docs.typesafe.ai/primitives/noul.md)).
- **`choice`**:
  - `type:"choice"`, `instructions`, required `criteria: map<option, EntryType|null>`, **max 255 options**.
  - Answer: `{type, choice, probabilities (sum 1), confidence}`
    ([choice](https://docs.typesafe.ai/primitives/choice.md)).
- **`score`**:
  - `type:"score"`, `instructions`, required `criteria: array<EntryType>`, 2-10 ordered levels.
  - Answer: `{type, score (probability-weighted mean, can fall between levels), legend, probabilities, confidence}`
    ([score](https://docs.typesafe.ai/primitives/score.md)).

The Python SDK types `instructions` as optional (`JSONContent | None = None`), while the HTTP reference marks it
required ([Python types](https://docs.typesafe.ai/sdk/python/api/types/questions.md) vs
[API ref](https://docs.typesafe.ai/api.md)). This is minor doc drift. Always send it.

`instructions` and every criteria value may be a **string, object or array** ("EntryType"). The object field names
are free-form and none are reserved ([advanced structure](https://docs.typesafe.ai/primitives/advanced.md);
[choice](https://docs.typesafe.ai/primitives/choice.md)).

Response top level: `model` (the versioned ID that answered, e.g. `jev-1.13.0`), `answers`, and
`usage: {input_tokens, output_tokens}` ([API ref](https://docs.typesafe.ai/api.md)).

The following types do **not** exist: extraction, span, free text, multi-label, or rationale. The three types above
are the complete list ([introduction](https://docs.typesafe.ai/introduction);
[primitives](https://docs.typesafe.ai/primitives.md)). Jev "does not generate text, write code, or hold a
conversation" ([coding agents](https://docs.typesafe.ai/introduction/coding-agents.md)).

You can emulate multi-label with one `noul` per label, since each noul is absolute and independent
([primitives](https://docs.typesafe.ai/primitives.md);
[jaggedness #8](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)).

## 2. Spans, quotes and reasoning

- **Jev does not return free spans or quotes.** "Generation" is a listed weakness. The advice is to "extract
  candidates with regex or a generative model, then let jev pick one via a Choice"
  ([jaggedness #9](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)).
- **Span-by-selection is documented twice:**
  - *Line-by-line search* ([cookbook](https://docs.typesafe.ai/cookbooks/semantic_find.md)):
    - Setup: the document is split into 218 lines and rendered into the state as `L000| …`. A `choice` question
      `where` has criteria `{L000: null, …}`, and a `noul` question `exists` asks "Does any line … address …".
    - Lesson: the choice always ranks *some* line first even when nothing matches, while the noul "can drop close to
      zero". Use both.
    - Thresholds used: FOUND 0.7, ABSENT 0.35.
    - Scale: a 255-option cap means longer documents need two passes (window first, then line).
  - *Pre-parsed value extraction* ([cookbook](https://docs.typesafe.ai/cookbooks/pre_parsed_value_extraction_cookbook.md)):
    regex candidates become `choice` options, plus a `"none"` key described as "None of these is the requested
    value". The model "cannot invent a value". The page reports single worked examples only, with no benchmark.
- **Per-option reasoning is not exposed.** The only per-option output is `probabilities`
  ([choice](https://docs.typesafe.ai/primitives/choice.md)). Rationales or explanations are not mentioned
  ([how to build](https://docs.typesafe.ai/concepts/how-to-build-with-system-one.md)).

`[INFERRED]` For our requirement, this means Jev's claim span can only be a unit that code segmented in advance
(line, sentence or paragraph). Phase 2's injection wants "<claim>". A whole line or sentence copied verbatim may be
longer or shorter than the ideal claim. Whether line granularity still gives phase 2's 0/9-0/10 effect is
**untested**.

## 3. State length, truncation and "none on 10/10"

- Documented limits ([models](https://docs.typesafe.ai/models.md)):
  - 64k tokens per request (state plus all questions);
  - 32k tokens for `state` plus the single longest question.
- Not documented: what happens when a request exceeds a limit (error, 422 or truncation), and at which end
  truncation would occur. The exceptions page lists 400/422 but ties neither to length
  ([exceptions](https://docs.typesafe.ai/sdk/python/api/exceptions.md)). The state page has no size guidance
  ([state](https://docs.typesafe.ai/concepts/state.md)).
- Related weaknesses: "Large state with irrelevant detail" acts as a distractor, and "Jev suffers from context rot".
  The advice is to filter in code or use a `noul` relevance filter first
  ([jaggedness #5](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)).
- **Could truncation explain "none on 10/10"?** `[INFERRED]` Probably not, if the state was only the draft: 5-30 KB
  is about 1.5k-8k tokens, well under 32k. It could matter only if the state also carried session context. I could
  not find the script that fed the fork-route real drafts to Jev: `phase1-rule-selection.py` handles only the 21-case
  corpus, so how that state was built is unknown to me. Cheap check: our `jev_select` discards `usage` and `model`.
  Logging `usage.input_tokens` per call ([API ref](https://docs.typesafe.ai/api.md)) would settle this without
  guessing.
- **More likely causes, from the docs** `[INFERRED mapping; each weakness is documented]`:
  1. **The question is compound.** "Which of 22 rules does this text violate?" is many judgments behind one answer,
     which TypeSafe calls the key anti-pattern
     ([how to build](https://docs.typesafe.ai/concepts/how-to-build-with-system-one.md);
     [primitives](https://docs.typesafe.ai/primitives.md)).
  2. **A choice is relative.** Across a long draft, most text truly "violates none of these rules", so `none` is the
     best *relative* description of the state. The 182-skill cookbook deliberately has **no none option** and uses
     `noul` gates instead ([skill suggestion](https://docs.typesafe.ai/cookbooks/skill_suggestion.md)).
  3. **Context rot and distractors.** One violating sentence sits in several KB of neutral text
     ([jaggedness #5](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)).
  4. **Literal reading and indirection.** Our rule texts are abstract laws ("A count must arrive with its unit"),
     while drafts instantiate them concretely, which requires a hop
     ([jaggedness #1, #4](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)).

## 4. Wording guidance, option count, `none`, imbalance and examples

- **Atomic questions.** A question should be a judgment a knowledgeable person makes "in about a second". Put the
  full question in `instructions`, since IDs are not sent ([primitives](https://docs.typesafe.ai/primitives.md)).
  Phrase it so that high means yes, and avoid inverted or contradictory criteria
  ([noul](https://docs.typesafe.ai/primitives/noul.md);
  [jaggedness #7](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)).
- **Option count.** Up to 255, and each option costs "only a few tokens". The docs advise giving the full list
  rather than a shortlist ([choice](https://docs.typesafe.ai/primitives/choice.md)). No weakness is listed for
  many options ([jaggedness](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)).
- **Option descriptions.**
  - Write them to "separate the options from each other".
  - If options get confused, use object form with `what`, `not_for` and `examples`
    ([choice](https://docs.typesafe.ai/primitives/choice.md);
    [advanced](https://docs.typesafe.ai/primitives/advanced.md)).
- **`none`.** Add "an `other` or `none of the above` option" if the list may not cover every input
  ([choice](https://docs.typesafe.ai/primitives/choice.md)). The skill cookbook shows the alternative: no `none`,
  with separate noul gates. There, the choice decides *which* option and the nouls decide *whether*
  ([skill suggestion](https://docs.typesafe.ai/cookbooks/skill_suggestion.md)).
- **Class imbalance.** Not documented ([choice](https://docs.typesafe.ai/primitives/choice.md)).
- **Few-shot.** There is no few-shot or exemplar parameter. Examples go **inside** criteria objects, e.g.
  `examples` arrays in choice options and in noul `true`/`false`
  ([advanced](https://docs.typesafe.ai/primitives/advanced.md)). The score page reports that a *relevant* example
  moved a result from 1.43/0.35 to 1.03/0.96, while an unrelated one changed nothing. "Examples only help when they
  resemble your real inputs" ([score](https://docs.typesafe.ai/primitives/score.md)).
- **Questions written by agents.** "Agents aren't great at writing questions, so expect to edit collaboratively"
  ([agent skill](https://docs.typesafe.ai/agent-skill.md)).

## 5. Calibration, `probabilities` and `confidence`

- `probabilities` is a distribution over options or levels that sums to 1
  ([API ref](https://docs.typesafe.ai/api.md)).
- `confidence` is "a statistic computed from the probability distribution" and describes how peaked it is. The
  production formula is not documented. The demo uses (n·peak−1)/(n−1), clamped
  ([confidence](https://docs.typesafe.ai/confidence.md)).
  - Confidence 1.0 "does not guarantee correctness" ([score](https://docs.typesafe.ai/primitives/score.md)).
  - To feed a statistical algorithm, "use probabilities instead of confidence"
    ([agent skill](https://docs.typesafe.ai/agent-skill.md)).
- **Calibration claim.** The model is trained with RLCD so that "higher probability should correspond to a greater
  chance that the answer is correct" ([ML primer](https://docs.typesafe.ai/introduction/machine-learning-primer.md)).
  - The primer shows **no evidence**: no reliability diagram, no ECE, no benchmark (same source).
  - A third-party article notes no calibration curve or paper has been released
    ([Cherry Creek News](https://thecherrycreeknews.com/typesafe-jev-system-one-model-claims-evals-independent-tests-cherry_creek/)).
    Treat that source as low-trust; its own caveat says so.
- **Thresholds.**
  - Noul: 0.5 when error costs are equal; raise the threshold when a false yes is costly; use a middle band for
    review (the example uses 0.8/0.2) ([noul](https://docs.typesafe.ai/primitives/noul.md)).
  - Confidence tiers are illustrative only: <0.5 goes to a human, >0.9 for high stakes
    ([confidence](https://docs.typesafe.ai/confidence.md)).
  - "Plot confidence against accuracy on your own data"
    ([how to build](https://docs.typesafe.ai/concepts/how-to-build-with-system-one.md)).
  - Pin a version if you tune thresholds ([models](https://docs.typesafe.ai/models.md)).
- **Structural invariants do not hold across questions.**
  - The same question asked as a noul gave 0.22, but as a choice gave 0.01.
  - A question and its negation summed to 1.19.
  - "Don't reuse a Noul threshold for a Choice"
    ([jaggedness #8](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)).
- **Non-determinism.** Our own measurement found the distribution differed on all 42 states over 3 runs, with the
  top pick stable on 33/42 (scoring doc). TypeSafe's own consistency cookbook is consistent with that:
  - Over 15 runs, it saw raw flips on 2 of 8 questions (Harassment 11 / Violence 4) and a mean probability std of
    0.0098.
  - The probe varied a `uid` field in the state, so it "cannot separate sensitivity to the irrelevant field from
    variation on identical requests"
    ([consistency: choices](https://docs.typesafe.ai/cookbooks/consistency_choice_cookbook.md)).
  - No seed or temperature control is documented.

## 6. Customisation, deployment and data

- **No fine-tuning or LoRA.** "All accounts share the same weights." Customisation is done through `state`,
  `instructions`/`criteria`, and atomic splitting ([models](https://docs.typesafe.ai/models.md)).
- **Self-hosting, private deployment or weights:** not documented anywhere I read
  ([legal](https://docs.typesafe.ai/legal.md); [models](https://docs.typesafe.ai/models.md)).
  - The GitHub org has SDKs, a `skills` repo, and `system-one-adapter-python`, described as a "Drop-in
    TypeSafeClient replacement backed by LLM APIs". It holds no weights ([GitHub](https://github.com/typesafe-ai)).
  - Service hosted on the US West Coast; early access with a waitlist
    ([launch blog](https://typesafe.ai/blog/introducing-system-one-models-and-jev)).
- **Data.**
  - Requests and responses are not used for training ([models](https://docs.typesafe.ai/models.md);
    [legal](https://docs.typesafe.ai/legal.md)).
  - **Zero data retention is enterprise-only**, via privacy@typesafe.ai.
  - The retention period otherwise defers to the DPA, which I did not read
    ([legal](https://docs.typesafe.ai/legal.md)).
  - `[INFERRED]` Sending drafts that contain repo content therefore needs an explicit provider-policy decision,
    consistent with tracker `0517d18ca272e132`.
- Third-party SEO sites claim "OpenJev"/"Laya" open clones and a "67.8% vs 74.1%" accuracy comparison
  (researcher sweep). **I could not verify these against any primary source and have excluded them.**

## 7. Latency, pricing, rate limits and versions

| item | value | source |
|---|---|---|
| model | `jev-1.13.0`; aliases `jev-latest` and `jev-preview` both → 1.13.0 | [models](https://docs.typesafe.ai/models.md) |
| other versions | `jev-1.12` appears in cookbooks; `GET /v1/models` lists aliases only, versioned IDs accepted | [models](https://docs.typesafe.ai/models.md), [semantic_find](https://docs.typesafe.ai/cookbooks/semantic_find.md) |
| price | $0.042 per M **input** tokens; output free | [models](https://docs.typesafe.ai/models.md) |
| rate limits | 250,000 tokens/s; 1,200 requests/min; 429 with `retry-after` | [models](https://docs.typesafe.ai/models.md) |
| latency | vendor figure "70ms-500ms" end-to-end, measured from US West Coast laptops; models page gives none | [launch blog](https://typesafe.ai/blog/introducing-system-one-models-and-jev) |
| observed latency in cookbooks | ~114 ms/call (consistency); 0.1-0.3 s (skill suggestion) | [consistency](https://docs.typesafe.ai/cookbooks/consistency_choice_cookbook.md), [skills](https://docs.typesafe.ai/cookbooks/skill_suggestion.md) |
| SDK default timeout | 10 s | [constants](https://docs.typesafe.ai/sdk/python/api/constants.md) |
| subsidy | vendor: "We can't prove it isn't subsidized" | [launch blog](https://typesafe.ai/blog/introducing-system-one-models-and-jev) |

Batching: questions are evaluated in parallel over a state that is ingested once. Extra questions "barely affect
latency" and cost only their own tokens. There is **no documented maximum question count**
([primitives](https://docs.typesafe.ai/primitives.md); [fan-out](https://docs.typesafe.ai/patterns/fan-out.md)).

## 8. Recommended configuration (documented fields only)

Principle: turn our one compound relative question into **per-rule atomic questions plus a locator**, over a
line-numbered state. This follows the documented line-by-line pattern and the "choice decides which, noul decides
whether" pattern.

**Preprocessing (code):**
- Split the draft into sentences or lines. Render each as `S000| …` in the state, or as a JSON object keyed by ID.
- If there are more than 255 units, window them: either run one call per window, or run a first pass with a
  `choice` over windows ([semantic_find](https://docs.typesafe.ai/cookbooks/semantic_find.md)).
- Keep the state to the draft only (no session context), to avoid context rot
  ([jaggedness #5](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)).
- `[INFERRED]` Sentence units are closer to "the claim" than lines. Markdown tables and code fences need a
  segmentation rule of our own.

**One request per draft (or per window)**, pinned to `jev-1.13.0`. The body below uses one rule for illustration and
repeats the pair of questions for each of the ~12 rules:

```json
{
  "model": "jev-1.13.0",
  "state": {
    "draft": "S000| I ran the lean lane and the default lane.\nS001| 4 tests failed.\nS002| ..."
  },
  "questions": {
    "makes::count_unit": {
      "type": "noul",
      "instructions": {
        "question": "Does any sentence in `draft` state a count of a defect population without naming the unit being counted?",
        "focus": "A bare number of bugs, sessions, peers, instances or tests, with no unit or scope."
      },
      "criteria": {
        "true": {"what": "At least one sentence gives such a bare count.", "examples": ["<real positive sentence from our corpus>"]},
        "false": {"what": "Every count names its unit and scope, or there is no count.", "examples": ["<its corrected twin>"]}
      }
    },
    "where::count_unit": {
      "type": "choice",
      "instructions": "Which sentence of `draft` states a count of a defect population without naming its unit?",
      "criteria": {"S000": null, "S001": null, "S002": null}
    }
  }
}
```

All field names are documented: `model`, `state`, `questions`, `type`, `instructions`, and `criteria` with
`true`/`false` for a noul or a map for a choice. The `question`/`focus`/`what`/`examples` keys inside objects are
free-form keys, as the docs show ([advanced](https://docs.typesafe.ai/primitives/advanced.md)).

**Decision (code):**
- For each rule, fire when `noul ≥ τ_r`. Then claim = the text of `argmax(where.probabilities)`. Log the top-3
  sentences for the verifier.
- Do not add `none` to the `where` choice. The noul already answers "whether"; this is the semantic_find and skill
  cookbook design. The `where` answer for a rule whose noul is low is simply ignored ("speculative fan-out",
  [fan-out](https://docs.typesafe.ai/patterns/fan-out.md)).
- Average ≥3 runs, since output is non-deterministic.
- Tune `τ_r` per rule on held-out pairs. Never reuse a noul threshold for a choice
  ([jaggedness #8](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)).
- `[INFERRED]` "Any sentence … states X" is more local and less compound than "the text violates rule R", and
  wording it as the rule's *claim-shape* (as the phase-0 detectors do) avoids the law-to-instance hop.

**Budget estimate** `[INFERRED]`:
- 12 rules × 2 questions. Each `where` lists up to 255 null-described IDs at "a few tokens" each, so about ≤1k
  tokens per question, about 12k in total. The draft adds ≤8k.
- That is about 20k tokens per request: under 64k total, and well under 32k for state plus the longest question.
- Cost ≈ 20k × $0.042/M ≈ **$0.0008 per call**. Three runs cost about $0.0025 per draft.
- Rate limits are irrelevant at our volume.

**Variants worth registering as arms:**
- **V1 (above):** per-rule noul plus a per-rule line locator.
- **V2 (chunked):** each paragraph becomes its own state with only the per-rule nouls; the chunk is the span. This
  reduces distractors, but loses cross-chunk claims. `[INFERRED]` The contradiction tell needs two claims, so it is
  a poor fit for chunking.
- **V3 (skill-cookbook two-stage):** Call 1 is a choice over rules with no `none`, used as a ranking, plus noul
  gates. Call 2 re-judges the top-k with full rule text plus `fits::` nouls
  ([skill suggestion](https://docs.typesafe.ai/cookbooks/skill_suggestion.md)). Our measured top-3 is 53%, so recall
  in stage 1 is the risk. Because nouls are cheap, V1's "all rules" coverage is preferable.
- **V4 (localiser only):** Jev's `where` ranks the top-k sentences per rule, and the Haiku judge verifies only those
  sentences. It still outputs YES/NO plus a verbatim quote, which can be forced to be one of the candidates.

## 9. Honest comparison with the Haiku per-rule judge

| dimension | Jev (configured as §8) | Haiku per-rule judge |
|---|---|---|
| claim span | only a code-segmented unit (line/sentence), copied verbatim; cannot trim or rephrase ([jaggedness #9](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)) | free verbatim quote (must be validated as a substring) |
| rule-shape reasoning | documented weak on literal reading, indirection, counting, dates ([jaggedness](https://docs.typesafe.ai/model-jaggedness/jev-1.13.md)); many of our rules are about counts, units, derivations: `[INFERRED]` a bad overlap | can reason; our Phase 1A saw it over-fire on clean text (4/6 gate) |
| measured on our data | choice: top-1 24%, top-3 53%, `none` on 10/10 real drafts; noul gate 3/5 (near-misses at 0.68 and 0.93) | reasoned Haiku: top-1 14% on the menu; 0.00 on the RTD-8 near-miss (10 runs) |
| cost | ~$0.001/draft `[INFERRED estimate]`; paid API, US-hosted, ZDR enterprise-only | subscription, no marginal $; rate-limited |
| latency | vendor 70-500 ms per call, all rules in one call | seconds per rule call × ~12 rules `[INFERRED]` |
| determinism | non-deterministic, no seed control | temperature not controllable via `claude -p` `[INFERRED]` |
| data policy | third-party API; needs explicit approval | stays within existing subscription channel |

**Verdict** `[INFERRED, grounded in the above]`:
- **Jev is not a good fit as the sole phase-1 selector for the claim-span requirement.** Structurally it cannot
  produce a claim, only pick a pre-cut unit. Its documented weaknesses coincide with the rule types we police.
  And our only per-rule (noul) measurement already missed the registered gate.
- **Where it might add value:** as a cheap, fast *localiser and prefilter* (V4). It narrows a 5-30 KB draft to 1-3
  candidate sentences per rule, and Haiku then judges and quotes only those. This could reduce Haiku's over-firing on
  clean text, which is the Phase 1A gate failure, and cut Haiku's context.
- **Whether V1 or V4 beats Haiku-only is an empirical question.** Register it and compare within one route and one
  judge channel, per the standing constraints. The first measurement needs no new mechanism: log
  `usage.input_tokens` and `model` in `jev_select`.
- **For an apples-to-apples arm on one interface:** `system-one-adapter-python` claims to back the TypeSafe client
  with LLM APIs ([GitHub](https://github.com/typesafe-ai)). I did not verify that it supports Anthropic models or
  the subscription channel.

## Not documented (checked, silent)

- What happens when input exceeds 32k/64k (error vs truncation, and from which end).
- The exact production formula for `confidence`; any calibration evidence (ECE, reliability curves).
- A maximum number of questions per request.
- Seed, temperature or determinism controls; any few-shot or exemplar parameter.
- Class-imbalance guidance.
- Self-hosting, private deployment, data residency; the retention period outside ZDR (deferred to the DPA, which I
  did not read).
- Latency on the models page; a deprecation policy for versions.
- Any span, extraction, rationale or per-option reasoning output.

## Sources (primary)

- https://docs.typesafe.ai/introduction
- https://docs.typesafe.ai/llms.txt
- https://docs.typesafe.ai/api.md
- https://docs.typesafe.ai/primitives.md
- https://docs.typesafe.ai/primitives/choice.md
- https://docs.typesafe.ai/primitives/noul.md
- https://docs.typesafe.ai/primitives/score.md
- https://docs.typesafe.ai/primitives/advanced.md
- https://docs.typesafe.ai/concepts/state.md
- https://docs.typesafe.ai/concepts/how-to-build-with-system-one.md
- https://docs.typesafe.ai/introduction/coding-agents.md
- https://docs.typesafe.ai/introduction/machine-learning-primer.md
- https://docs.typesafe.ai/confidence.md
- https://docs.typesafe.ai/models.md
- https://docs.typesafe.ai/model-jaggedness/jev-1.13.md
- https://docs.typesafe.ai/legal.md
- https://docs.typesafe.ai/agent-skill.md
- https://docs.typesafe.ai/patterns/fan-out.md
- https://docs.typesafe.ai/cookbooks/semantic_find.md
- https://docs.typesafe.ai/cookbooks/pre_parsed_value_extraction_cookbook.md
- https://docs.typesafe.ai/cookbooks/skill_suggestion.md
- https://docs.typesafe.ai/cookbooks/citation_check.md
- https://docs.typesafe.ai/cookbooks/llm_guardrails.md
- https://docs.typesafe.ai/cookbooks/consistency_choice_cookbook.md
- https://docs.typesafe.ai/sdk/python/api/types/questions.md
- https://docs.typesafe.ai/sdk/python/api/exceptions.md
- https://docs.typesafe.ai/sdk/python/api/constants.md
- https://typesafe.ai
- https://typesafe.ai/blog/introducing-system-one-models-and-jev
- https://github.com/typesafe-ai

Secondary (low trust, used only for the calibration-evidence remark):
https://thecherrycreeknews.com/typesafe-jev-system-one-model-claims-evals-independent-tests-cherry_creek/
