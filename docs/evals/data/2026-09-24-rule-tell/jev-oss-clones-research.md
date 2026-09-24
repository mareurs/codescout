# Open-source Jev ("System 1") clones: can we fine-tune one for phase-1 rule + claim-span selection?

Date: 2026-09-24. This is web and docs research only: no weights were downloaded, nothing was installed or trained, no paid API was called, Jev was not called, and no key was read.
It builds on `jev-research.md` in this directory (Jev's API, its limits, the sentence-ID trick) and does not repeat it.
`[INFERRED]` marks my reasoning. "Not found" means I looked and found nothing.

In-repo context I read:
- the phase-1 handoff (`docs/evals/rule-tell-scoring-2026-09-23.md` § Next — phase 1);
- the Phase-1 amendment (`docs/evals/rule-injection-timing-preregistration.md:286ff`);
- the design tracker `d16552e9981f521e` (§ The ladder, § Learning material and evaluation);
- the evidence review `docs/research/2026-09-18-local-semantic-evaluator-review.md`;
- `scripts/phase1-rule-selection.py` (`OPTIONS` :46, `GOLD` :78);
- `scripts/phase1-span-selector.py` (outline);
- `docs/evals/rule-tell-detection.md` (distribution, case format).

---

## TL;DR

1. **The leaderboard in the screenshot is stale.** It is JevBench **v1.2** (19 Sep). The current release is **v1.4.1** (23 Sep). It adds 308 sealed decisions and switches to a harmonic mean. Now:
   - SemIf is **#9 (47.7)**.
   - djev is **#6 (52.2)**, and its hosted API is paused.
   - The top open entries are **JevK5 v0.2.0 (#2, 62.0)** and **Hopper (#3, 59.4)**, both Qwen3.5-4B plus a LoRA.
2. **The key number in the whole benchmark is the sealed accuracy.** On fresh, never-published decisions, *every* Jev-class system is near chance: Jev 36.7%, JevK5 33.1%, Hopper 34.1%, SemIf 26.3%, against a **29.3% chance** baseline. Public-set accuracy for the same systems was 80–87%. `[INFERRED]` Fast logit-readout readers look strong on public items and collapse on novel hard judgments. Our 24% top-1 and `none`-on-10/10 results are consistent with this, so no off-the-shelf clone will fix our problem.
3. **The best base to fine-tune is Qwen3.5-4B** (Apache-2.0, 262k native context). The best starting checkpoint is **JevK5**:
   - Apache-2.0 code and weights;
   - published `training/teacher.py` and `training/lora.py`;
   - accepts inputs up to 16,384 tokens, which covers our 1.5k–8k-token drafts;
   - about 9 GB of VRAM, which fits our RTX A5000 (24 GB).

   **Kev** is the most complete fine-tune toolkit (`--init_from`), but it was trained on ≤384-token states.
4. **For our task I would not reuse the clones' output shape.** A choice among ≤16 option letters is the wrong interface for "which of about 22 rules, and which sentence". I recommend one forward pass over the sentence-segmented draft, with a **sentence × rule multi-label head** (22 logits per sentence). The span is then the chosen sentence, verbatim by construction, and the draft-level decision is a max or noisy-OR over sentences. This is the LettuceDetect token/span-classification pattern, applied to rules. Keep the verbatim check.
5. **Data is the binding constraint, not compute, and it carries a policy problem:**
   - We have 21 pairs; the smallest published clone recipe used about 2.7k examples.
   - Anthropic's Usage Policy prohibits "Utilization of inputs and outputs to train an AI model (e.g., 'model scraping' or 'model distillation')" without prior authorization. That rules out distilling Haiku labels. `[INFERRED]` It plausibly also covers training on Claude-authored drafts, which are *our entire input distribution*. This needs an explicit decision before any training.
6. **Recommendation:** build it as a **prefilter in front of Haiku**, not a replacement. Register four kinds of arm, each measured against Haiku-only on Score A and Score B, plus Haiku token spend:
   - zero-shot local arms (cost nothing and settle whether the base has any signal);
   - a MiniLM/ModernBERT head;
   - a Qwen3.5-4B LoRA head;
   - a cascade.

---

## 1. JevBench

**What it is and who runs it.**
- JevBench is Benchmark Heaven's benchmark of "Jev-class decision models: state and a bounded rubric in, a typed answer out." It is "not affiliated with or endorsed by TypeSafe AI" ([Benchmark Heaven](https://benchmarkheaven.com/jev-models); [repo README](https://github.com/fstandhartinger/jevbench)).
- The author calls it "a one-person hobby project" that he pays for himself, through productivity-boost.com Betriebs UG ([README](https://github.com/fstandhartinger/jevbench)).
- Not to be confused with `jevbench.dev`, which is an unrelated StarCraft-II agent benchmark ([jevbench.dev](https://jevbench.dev/leaderboard)).

**Where the harness, tasks and data live.** [github.com/fstandhartinger/jevbench](https://github.com/fstandhartinger/jevbench), MIT (harness, 72 original public decisions, scoring rules). It contains:
- `datasets/public/{original,easy,hard}.jsonl`;
- the `jevbench/` harness with adapters `typesafe`, `systemone_list`, `gradio_space`, `local_openjev` and `openai_compat`;
- `results/v1.x/` and the method document, METHOD-v1.4 ([README](https://github.com/fstandhartinger/jevbench); [method](https://github.com/fstandhartinger/jevbench/blob/main/docs/METHOD-v1.4.md)).

**Data.**
- v1.0 had 242 decisions (72 hand-written public, 24 held out, 146 imported from the author's router experiment).
- v1.1 added an easy tier.
- v1.2 added 220 hard decisions "written by Claude Opus 5 and GPT-5.6 Sol, cross-reviewed, frozen and hashed", for **534** per system.
- v1.4 added **308 sealed** decisions, published as aggregates only ([README](https://github.com/fstandhartinger/jevbench)).
- The sealed families are "temporal and numeric decisions, subtle answer judgment, long policies, multi-hop lookup, abstention, probability, constrained tradeoffs, safety judgment, paraphrase sensitivity and adversarial traps" ([method](https://github.com/fstandhartinger/jevbench/blob/main/docs/METHOD-v1.4.md)).

**How the axes are measured.** v1.3 definitions, carried into v1.4 ([README](https://github.com/fstandhartinger/jevbench); [method](https://github.com/fstandhartinger/jevbench/blob/main/docs/METHOD-v1.4.md); [leaderboard](https://benchmarkheaven.com/jev-models)):

| axis | definition |
|---|---|
| Intelligence | Chance-corrected accuracy `(acc−chance)/(1−chance)`, tier weights easy .14 / standard .28 / judge .28 / hard .30. v1.4: `I = 0.8·I13 + 0.2·I_sealed`, with `I_sealed = 100·max(0,(a−0.293)/(1−0.293))`. Multiplied by `1 − max(0, gap−25)/100`, where gap = public − sealed accuracy in percentage points. |
| Calibration | ECE on the hard tier plus fidelity to the gold distributions. "Label-only systems score 0." Pooled ECE is named, and Brier is reported separately. |
| Speed | `100 − 20·log10(s/0.1 s)`, averaged over p50 and p95. Self-hosted systems are adjusted "×2 + 0.15 s". |
| Cost | `100 − 30·log10($ per 1k decisions / $0.001)`, where cost = list price × measured tokens. Self-hosted weights are priced at the OpenRouter/DeepInfra list price for the same weights, never at GPU rental. |
| Composite | v1.3: a 25% geometric mean. v1.4: an **equal-weight harmonic mean**, with an `(axis/50)²` multiplier on I, S and K below 50. |

- Probabilities are read "native" from the model's own distribution, or "verbalized" for `openai_compat`. "Token-level logprobs are not used anywhere, for anyone" ([README](https://github.com/fstandhartinger/jevbench)).
- Worked cost example: Jev at 950 input tokens × $0.042/M = $0.0399 per 1k decisions ([leaderboard](https://benchmarkheaven.com/jev-models)). `[INFERRED]` Typical JevBench items are therefore about 1k tokens. Our 1.5k–8k-token drafts are *outside* the benchmark's distribution.

**Is it reproducible locally?**
- **Public tiers: yes.** Requirements are Python 3.10+, stdlib-only HTTP adapters, and `python -m jevbench.cli run --tasks … --adapter typesafe …` against any `/v1/systemone` server. `--key-env ''` sends no auth. The harness keeps a budget ledger and stops on 401/403/429 ([README](https://github.com/fstandhartinger/jevbench)).
- **Sealed tier: no.** It is private by design ([method](https://github.com/fstandhartinger/jevbench/blob/main/docs/METHOD-v1.4.md)).

**Can we add our own tasks?**
- **Publicly: there is no documented procedure.** The README "does not describe any procedure for contributing new tasks". Only systems are submitted, via issues ([README](https://github.com/fstandhartinger/jevbench); [leaderboard](https://benchmarkheaven.com/jev-models)).
- **Privately:** `[INFERRED]` Yes. The harness reads JSONL task files, so a private `datasets/private/rules.jsonl` of `choice`/`noul` items could be run through it with the same `typesafe` adapter against local servers.
- **The limit:** JevBench scores typed answers only, so it cannot score the claim span, which is the thing we need.
- **A better public target is DecisionBench** (Hanno Labs, Apache-2.0). It explicitly accepts "a dataset-backed decision problem with labels, provenance, and tests" ([decision-bench](https://github.com/Hanno-Labs/decision-bench)).
- `[INFERRED]` Publishing our cases anywhere would (a) expose repo text and (b) contaminate our own held-out set, so do not.

**Cautionary finding.** Several authors disclose repeated development against the public items:
- reflex used the public items "four times as a development gate";
- Hopper used "26 model/prompt configs, 20+ calibration-map variants";
- metask built "synthetic families that deliberately mirror JevBench's hard families".

Every one of them lost about 50 pp on the sealed set ([v1.4 results JSON](https://raw.githubusercontent.com/fstandhartinger/jevbench/main/results/v1.4/jevbench-v1.4-results.json)). This is the leakage lesson for §4.

## 2. Jev clones and competitors on JevBench

Current ranks are v1.4.1 ([leaderboard](https://benchmarkheaven.com/jev-models)). Public and sealed accuracy come from the [v1.4 results JSON](https://raw.githubusercontent.com/fstandhartinger/jevbench/main/results/v1.4/jevbench-v1.4-results.json). The v1.2 ranks in the screenshot are superseded.

### 2a. The ones that matter for us (detail)

| system (rank v1.4.1) | architecture / base | licence | weights | API | probabilities | context | hardware / latency | training recipe published? |
|---|---|---|---|---|---|---|---|---|
| **Jev 1.13.0** (#1, 63.3; public 86.6% / sealed 36.7%) | closed | proprietary | none | `/v1/systemone` choice/score/noul | "native", RLCD-trained (previous report) | 32k state + question | p50 0.65 s | no ([previous report](jev-research.md)) |
| **JevK5 v0.2.0** (#2, 62.0; 85.3% / 33.1%) | Qwen3.5-4B + LoRA r16 on attention, merged bf16; distilled from Qwen3.6-27B-thinking | Apache-2.0 (code + weights); prompt/readout adapted from SemIf (MIT) | [HF alibiserikbay/JevK5](https://huggingface.co/alibiserikbay/JevK5), plus `-GGUF` and a `-2B` variant | TypeSafe `/v1/systemone` shape | softmax over option-letter logits at the last position, one temperature T=1.532 | **refuses >16,384 tokens**; ≤16 options | ~9 GB VRAM bf16; ~13 ms/decision on H100 with CUDA graphs | **yes**: `training/teacher.py`, `training/lora.py`; 2 epochs, lr 3e-5; data 3,272 teacher + 3,272 human-labelled items ([repo](https://github.com/allebee/jevk5); [issue #31](https://github.com/fstandhartinger/jevbench/issues/31)) |
| **Hopper** (#3, 59.4; 82.3% / 34.1%) | Qwen3.5-4B + LoRA r16 (PEFT) | Apache-2.0 | [HF HopitAI/hopper](https://huggingface.co/HopitAI/hopper) | `/v1/systemone` | letter-logit softmax plus per-type temperature map (choice 0.790, noul 0.753, score 0.900) | not stated; "weak on long, multi-hop documents" | needs `flash-linear-attention` or ">10x slower"; VRAM not stated | serving code only; data sources listed, sizes and hyperparameters not stated ([card](https://huggingface.co/HopitAI/hopper)) |
| **Winnow-12B Q8** (#4, 55.6; 85.7% / 33.1%) | 12B GGUF (Gemma-3-12B reference for pricing) | not found | not found (model card not found) | `/v1/systemone` server; run at 8,192 context | native | 8,192 in the benchmark run | ~13 GB Q8 file `[INFERRED]`; p50 0.23 s | no: "private training corpus was not released" ([leaderboard](https://benchmarkheaven.com/jev-models); [search summary of issue](https://github.com/EldanRing/winnow-inference/issues/1)) |
| **reflex 4B** (#5, 54.0; 79.2% / 28.3%) | Qwen3.5-4B (+ optional LoRA) | MIT code + adapter; Apache-2.0 base | [HF kshetrajna12/reflex-qwen3.5-4b-lora](https://huggingface.co/kshetrajna12/reflex-qwen3.5-4b-lora) | `/v1/systemone` noul/choice (≤26 options)/score | label logits; trained with a **proper scoring rule** (log or Brier) on hard or soft labels; T fit | not stated | 8–16 GB GPU; ~200 ms | **yes**: `reflex-data`, `reflex-calibrate train`, `reflex-distill`. But the author says "**Every adapter trained in this repo was rejected**" because adapters lost judgement on long, ambiguous inputs ([repo](https://github.com/kshetrajna12/reflex)) |
| **djev** (#6, 52.2; 84.0% / 29.9%) | an inference method on `google/diffusiongemma-26B-A4B-it`, no own weights | Apache-2.0 code and base | Google's weights | `POST /v1/request` with noul/choice/score (not `/v1/systemone`); image inputs | "one denoising read" of allowed-label token probabilities | not stated | reference is 1× B200 BF16; historical 76.9 ms p50 on an older quantized config | no training ([djev-dev](https://github.com/Davipar/djev-dev)). Hosted API paused ([results JSON](https://raw.githubusercontent.com/fstandhartinger/jevbench/main/results/v1.4/jevbench-v1.4-results.json)). `[INFERRED]` 26B×2 bytes ≈ 52 GB of weights, which does **not** fit our 24 GB GPU |
| **SemIf** (#9, 47.7; 81.0% / 26.3%) | **frozen** Qwen3.5-4B (also 0.6B, 2B, Reranker-4B, 27B bridge) | MIT | none (uses base weights) | CLI `semif-score` over JSONL (`state`, `question`, `options`); no HTTP | option-logit readout, per-workload temperature (authored T=1.23) | not stated | GPU that holds 4B bf16; 21 criteria in 1.023 s on an RTX 3090 (vs 5.33 s generating JSON) | no training ([repo](https://github.com/TheoLeeCJ/SemIf)) |
| **Kev 4B** (#24, 36.1) | Qwen3.5-4B-Base + LoRA r16 (incl. DeltaNet projections) + **pointer head** over `</opt>` states | Apache-2.0 | [HF jaredpalmer/kev-4b](https://github.com/jaredpalmer/kev) | `/v1/systemone` noul/choice/score, 1–255 options | pointer-head softmax, T≈2.1–2.4 | **trained on ≤384 state tokens**, serves ≤8,192 | 4B ~1 h on 1× H100 to train; 36–179 ms on L40S | **yes, most complete**: `kev.train`, `--init_from`, a `kev-finetune` skill, locked test sets ([repo](https://github.com/jaredpalmer/kev)) |
| **Bespoke Nimble 9B** (#52, 18.7) | Qwen3.5-9B + LoRA r16 | Apache-2.0 | HF (via [repo](https://github.com/bespokelabsai/nimble)) | own flat enum/boolean schema | CE on allowed logits; "calibration is implicit" | 2,048-token training limit (benchmark re-run at 8,192) | 106 ms on H100 | **yes**: contrastive data curation, 2,676 examples, 1 epoch ([search summary](https://jevainews.com/news/bespoke-nimble/); [repo](https://github.com/bespokelabsai/nimble)) |

### 2b. The rest of the v1.4.1 board (brief)

From the [leaderboard](https://benchmarkheaven.com/jev-models) unless noted. Licences are mostly "check the repo" per the page.

- **Qwen3.5-4B family (native logits):**
  - Jev-Omni (Gemma-4-12B merged, #7);
  - metask-jev-4b (#8);
  - Jobe (frozen Qwen3.5-4B, no trained weights, #10);
  - local-jev (#11);
  - system-one-open (Gemma 4 E2B LoRA, #12);
  - spark-s1-4b-v6 (#13);
  - OpenSourceJev (Q4_K_M, #18);
  - open-alternative-jev (#31).
- **Larger bases:**
  - jqv (Qwen3-32B, #14);
  - decider-35b-a3b (#16);
  - SimpleJev (Qwen3.8-27B / 3.6-35B-A3B);
  - NInfer (Qwen3.8 27B / Flash-Next);
  - openjev-sglang (Qwen3.6-35B-A3B);
  - LitJev; reflex-27b;
  - Open-Jev 9B/2B (Zefan Cai).
- **Encoders and small models (cheap, weak intelligence):**
  - Laya (ModernBERT-large 421M, PPO head);
  - jeff (GLiFormer 400M);
  - Von (395M);
  - openJev Verdict (ModernBERT 151M);
  - open-jev-deberta-v3-large;
  - GLiNER2 / 2.5 (Fastino; a span extractor, Apache-2.0, [repo](https://github.com/fastino-ai/GLiNER2));
  - lev-350m (LFM2.5);
  - Decision 2B (MiniCPM5-2B);
  - Decision Fast (Qwen3-0.6B).
- **Closed or API:** decision-machine-1 (milliseconds.ai), JevOne (Juspay), GPT-6 Luna (I 95.8, but K 36.9), Gemini 3.1 Flash-Lite, DeepSeek V4.1 Flash. classifier.dev is unranked because its fast tier is Jev.
- **Controls:** raw Qwen3 4B Instruct 2507 (C 29.1), raw Phi-4 mini, rerankers (bge, mxbai, GTE: I ≈ 5).
- **Attribution conflict, recorded:** Latent Space attributes SemIf/OpenJev to "AlexWortega" (HF `AlexWortega/openjev`) and calls it a "three-class NLI classifier on the last token" ([Latent Space](https://www.latent.space/p/ainews-here-are-6-clones-of-jev-in)). The leaderboard and the repo credit Theodore Lee (TheoLeeCJ) and describe option-logit readout ([repo](https://github.com/TheoLeeCJ/SemIf)). I trust the repo.

**What the population says** `[INFERRED from the numbers above]`:
- The leaders converge on one recipe: Qwen3.5-4B, a rank-16 LoRA, cross-entropy on option-letter logits, one temperature. They differ mainly in data.
- None emits text.
- None was trained on inputs near our length: Kev ≤384 state tokens, Nimble 2,048, JevK5 capped at 16k (trained length not stated).
- **Sealed accuracy of 26–37% against a 29% chance baseline means none of them, Jev included, generalises to novel hard judgments.** Our rules are exactly "subtle answer judgment" on unseen text.

## 3. Fine-tuning feasibility for our task

### 3a. Hardware we have (measured, `nvidia-smi` / `free -g` / `lscpu`, 2026-09-24 09:09)

- 1× **NVIDIA RTX A5000, 24,564 MiB**, with ~3.6 GB used by the desktop, leaving ~20 GB usable.
- **125 GB RAM** (37 GB available at sample time; 3 GB swap fully used).
- **AMD Threadripper PRO 3975WX, 32 cores / 64 threads.**

### 3b. Best base model

**Base: Qwen3.5-4B**, 4B dense (the Hub shows 5B including the vision encoder). Why:
- **Hybrid architecture:** "8 × (3 × (Gated DeltaNet → FFN) → 1 × (Gated Attention → FFN))".
- **Context:** "262,144" native.
- **Licence:** Apache-2.0.
- **Thinking** can be disabled via `enable_thinking: False` ([model card](https://huggingface.co/Qwen/Qwen3.5-4B)).
- **Ecosystem:** it is the base of JevK5, Hopper, reflex, SemIf and Kev-4B, so we inherit readout code, calibration practice and known pitfalls.
- **Training tools:** Unsloth supports it, with example `r=16` on all projections ([Unsloth guide](https://unsloth.ai/docs/models/qwen3.5/fine-tune)).

**Pitfalls:**
- Hopper warns the model is ">10x slower" without `flash-linear-attention` ([card](https://huggingface.co/HopitAI/hopper)).
- Kev notes DeltaNet layers "ignore masks", so packing questions needs per-question rows ([repo](https://github.com/jaredpalmer/kev)).

**Starting checkpoint:**
- **JevK5** is the only clone that is fully Apache-2.0, has published training code, and accepts inputs of our length (≤16,384 tokens) ([repo](https://github.com/allebee/jevk5)).
- **Kev-4B** has the best fine-tune ergonomics. One user's test found that starting from the released checkpoint with `--init_from` kept 0.83 on Kev's own eval and reached 0.88 on the new domain, while fine-tuning from the base dropped to 0.33 ([repo](https://github.com/jaredpalmer/kev)). But it was trained at ≤384 state tokens.

**The cheaper alternative backbone: ModernBERT-large.** 395M parameters, 8,192 native context, Apache-2.0 ([card](https://huggingface.co/answerdotai/ModernBERT-large)). It is the LettuceDetect backbone, see 3d.

### 3c. Method

`[INFERRED design]`. The clones' interface (one question, ≤16–26 option letters) does not fit our dual output. The recommended shape:

1. Segment the draft into sentences with code (the rule we already need for the verbatim check, including a policy for tables and code fences). Insert a marker token after each sentence.
2. Run one forward pass of the backbone over the whole draft. `[INFERRED]` Rule texts can be prepended once as a shared prefix, or learned as per-rule head weights.
3. **Head:** for each sentence-marker hidden state, a linear layer to 22 logits (one per rule), each with its own sigmoid. This is multi-label per sentence and is the same pattern as LettuceDetect's per-token supported/hallucinated classifier ([LettuceDetect paper](https://arxiv.org/abs/2502.17125)) and Kev's pointer head ([repo](https://github.com/jaredpalmer/kev)).
4. **Draft-level decision:** P(rule r fires) = max, or noisy-OR, over sentences. Claim = the argmax sentence, copied verbatim. The existing `verify_span` (`scripts/phase1-span-selector.py:83`) becomes an invariant check rather than a filter.
5. **Training:** LoRA r16 plus the head, with per-(sentence, rule) binary cross-entropy (a proper scoring rule, per reflex's rationale, [repo](https://github.com/kshetrajna12/reflex)) and positive-class weighting, because positives are sparse.

**Per-rule binary heads versus multi-class.** Use per-rule sigmoid heads, not a softmax over rules:
- A draft can violate several rules. The corpus already gives two gold answers for three cases (`GOLD`, `phase1-rule-selection.py:78`).
- Jev's measured failure was the relative, softmax-like `choice` collapsing to `none` (previous report §3).
- Per-rule heads also let rules with too little data stay on Haiku while the others go local.

**Data volume needed.** `[INFERRED from precedents]`:
- Precedents: Nimble 2,676 examples, 1 epoch ([summary](https://jevainews.com/news/bespoke-nimble/)); JevK5 6,544 ([repo](https://github.com/allebee/jevk5)); Kev about 12.6k ([repo](https://github.com/jaredpalmer/kev)); reflex 800 per source ([repo](https://github.com/kshetrajna12/reflex)).
- For 22 rule heads, a floor of about **50–100 positive sentences per rule, plus 10× hard negatives** is plausible: roughly 1–2k positives.
- We have 21 pairs, and several rules have 1–4 cases each (`GOLD`). **No rule is trainable on the current corpus alone.**

### 3d. Can a generative 4B emit the span directly?

- **Evidence that fine-tuned small generative detectors localise spans well:** Kovács et al. (2026) fine-tune a **Qwen3.5-2B** span detector over code, tool output and markdown. It reaches span-F1 **0.60** on code-agent sources, against **0.17** for encoder LettuceDetect-large and "at most 0.22" for zero-shot LLM judges. The abstract does not describe how non-verbatim predicted spans are handled ([arXiv 2607.00895](https://arxiv.org/abs/2607.00895)).
- **Recommendation: keep sentence-ID selection plus the verbatim check as the primary path.** `[INFERRED]` Reasons:
  - Phase 2 tested *verbatim* claims, and a generated span can drift.
  - A classification head yields per-(rule, sentence) probabilities for calibration and thresholds; generation yields only token likelihoods of a string.
  - Selection costs one prefill with no decode loop. SemIf measured 1.02 s for logit readout against 5.33 s for generating the same JSON on a 3090 ([repo](https://github.com/TheoLeeCJ/SemIf)).
- **Optional stage 2:** generative *trimming* of the selected sentence to the minimal claim, verified as a substring. Register it as its own arm only if Score B shows that whole-sentence claims underperform the hand-trimmed 1b claims.

### 3e. Calibration after fine-tuning

- Fine-tuning, especially on small datasets, tends to make LLMs overconfident. The standard remedies are temperature scaling on held-out data (cheap; weaker under shift) and post-hoc Laplace-LoRA, which leaves the weights unchanged ([Laplace-LoRA, arXiv 2308.13111](https://arxiv.org/abs/2308.13111); [Guo et al.](https://proceedings.mlr.press/v70/guo17a.html)).
- **Clone practice:**
  - Every leader fits a single T post-hoc; the map never changes the argmax.
  - JevK5 fitted T on three held-out domains and found standard-tier answers came out underconfident, because it tuned on hard questions ([issue #31](https://github.com/fstandhartinger/jevbench/issues/31)).
  - reflex found "fitted temperatures don't transfer across distributions" and ships no calibration file in its stable configuration ([repo](https://github.com/kshetrajna12/reflex)).
  - reflex's in-distribution LoRA went from ECE 0.051 to **0.024** with temperature scaling ([repo](https://github.com/kshetrajna12/reflex)).
- **For us** `[INFERRED]`:
  - Fit **one temperature, or Platt a/b, per rule head** on a calibration fold kept separate from both train and eval.
  - Report per-rule ECE and Brier, with the bin count stated. Hopper found binned ECE over 55 items unstable ([card](https://huggingface.co/HopitAI/hopper)).
  - With our volumes, a reliability diagram per rule is not estimable; report pooled ECE plus per-rule Brier.
  - Refit whenever the model, prompt or precision changes ([reflex](https://github.com/kshetrajna12/reflex)).

### 3f. Footprint on our hardware (`[INFERRED]` unless cited)

- **Serving:**
  - Qwen3.5-4B in bf16 needs ~8.5–9 GB, per JevK5's measurement ([repo](https://github.com/allebee/jevk5)). That fits in ~20 GB free alongside the desktop.
  - One 8k-token prefill is ≈ 2 × 4e9 × 8e3 ≈ 6.4e13 FLOPs, so roughly **0.5–2 s per draft** on an A5000, for all 22 rules and all sentences at once. There is no per-rule call.
  - ModernBERT-large would be about 10× cheaper.
  - JevK5 also ships GGUF for CPU ([repo](https://github.com/allebee/jevk5)). `[INFERRED]` 32 cores make a CPU fallback viable at seconds per draft.
- **Training:**
  - Unsloth: "5GB VRAM to train Qwen3.5-2B LoRA", with batch 1, gradient checkpointing and reduced `max_seq_length` if OOM ([Unsloth](https://unsloth.ai/docs/models/qwen3.5/fine-tune)).
  - Kev-4B: ~1 h on an H100 at ≤1,024 tokens per example ([repo](https://github.com/jaredpalmer/kev)).
  - `[INFERRED]` 4B LoRA at 8k tokens, batch 1 with gradient checkpointing, should fit in 20 GB. 2k examples × 8k tokens × 2 epochs is on the order of **several hours to an overnight run on the A5000**.
  - ModernBERT-large at 8k: under an hour.
  - Marginal cost is electricity only.

## 4. Where training data would come from

| source | what it gives | volume today | notes |
|---|---|---|---|
| **21 RTD pairs** (`docs/evals/rule-tell-detection.md`) | gold (rule, claim sentence) positives plus their **corrected twins**, the ideal hard negatives | 21 positive / 21 negative texts; 1–4 per rule | **Must stay held out as Score A.** Never use as training seeds; see leakage below. Positives come from pre-correction blobs, since 7 of 21 corrections were appended, not applied in place (§ Distribution) |
| **Gate fixtures** (`GATE_CASES`, `phase1-rule-selection.py:235`) | known-answer texts | small | Held out: they are the gate |
| **57-passage control corpus** | clean negatives | 57 | Use for the false-positive rate. Split it: part calibration, part eval |
| **Real fork drafts** (`fork-dp1-n10.jsonl`, `fork-rtd3r.jsonl`) | in-distribution long drafts | 20 | Score B inputs, so held out |
| **Repo history mining** `[INFERRED, most promising]` | more self-corrections of the same shape as the RTD cases: appended retractions, "measured …" corrections, superseded counts | unknown, possibly hundreds | Needs the review doc's discipline: a frozen decision-time packet, no outcome prose in the input, incident-grouped splits, forward-in-time ordering (`2026-09-18-local-semantic-evaluator-review.md` § A feasible local evaluation) |
| **Synthetic, contrastive** | pairs identical except for one flipped fact (Nimble's curation, [summary](https://jevainews.com/news/bespoke-nimble/)), e.g. adding or removing the unit on a count, or the scope on a universal | unbounded | Generate with a **local open-weight teacher** (JevK5 used Qwen3.6-27B-thinking, Apache-2.0, [repo](https://github.com/allebee/jevk5)), with double-generation agreement filtering as JevK5 did. Human spot-check |
| **Hard-negative mining** | near-miss controls: the design tracker's "negative neighbours" (dated benchmark with a valid historical number; a narrow allowlist that is justified; top-N that advertises its limit) | from mining plus synthesis | Weight these heavily. They are exactly where Haiku over-fired (Phase 1A 4/6 gate) |
| **Distilled labels from Claude/Haiku** | cheap labels | n/a | **Blocked by policy; see below** |

**Anthropic terms: the constraint to settle first.**
- **Usage Policy** (effective 2025-09-15), Universal Usage Standards, "Do Not Abuse our Platform", prohibits "Utilization of inputs and outputs to train an AI model (e.g., 'model scraping' or 'model distillation')" "without prior authorization from Anthropic" ([AUP](https://www.anthropic.com/legal/aup)).
- **Consumer Terms** (the subscription channel we use; the page served was the EEA version, effective 2025-10-08), §3 prohibits using the Services "To develop any products or services that compete with our Services, including to develop or train any artificial intelligence or machine learning algorithms or models" ([Consumer Terms](https://www.anthropic.com/legal/consumer-terms)).
- **Commercial Terms** (effective 2025-06-17), §D.4: "build a competing product or service, including to train competing AI models" ([Commercial Terms](https://www.anthropic.com/legal/commercial-terms)).
- **Consequences** `[INFERRED; not legal advice]`:
  1. Haiku-judge labels used as training targets are squarely "outputs to train an AI model", so do not do this without authorization.
  2. Our drafts, the RTD positives, and the fork drafts are Claude-authored text. Training on them is arguably covered too, since the clause is not limited to competing models. That is the whole input distribution, so **this needs an explicit operator decision, or a request to Anthropic for authorization, before any training run.**
  3. Using Claude only to *evaluate* a local model (Score A/B scoring, fork re-runs) is not training. `[INFERRED]` Using Claude labels to fit calibration thresholds sits in a grey zone; treat it as training.
  4. **Clean fallback:** train only on human-written or open-weight-teacher text and labels. Accept the distribution shift, and measure it on the Claude-drafted held-out sets, which is evaluation, not training.

**Leakage controls** (from the review doc § A feasible local evaluation, plus the JevBench lesson):
- Group every version, twin and synthetic derivative of one incident into a single split. A synthetic variant seeded from RTD-8 is RTD-8 for split purposes.
- Split forward in time.
- Apply Hopper's overlap filter: drop any training item sharing normalised question identity or **any 8-word sequence** with an eval item ([card](https://huggingface.co/HopitAI/hopper)).
- Freeze and hash the eval sets before the first training run, as JevBench does for its sealed set ([method](https://github.com/fstandhartinger/jevbench/blob/main/docs/METHOD-v1.4.md)).
- Cap how many times any config is scored on the eval set. Log every evaluation, because the ~50 pp public-sealed gaps above are what repeated dev-gating produces ([results JSON](https://raw.githubusercontent.com/fstandhartinger/jevbench/main/results/v1.4/jevbench-v1.4-results.json)).
- Shared model lineage (a Qwen teacher and a Qwen student) is a leak of style, not of items. Disclose it (review doc).

## 5. How to improve on the clones for our case

- **Per-rule sigmoid heads over sentences, rather than letter-choice** (§3c). This removes the ≤16/≤26-option caps (JevK5, reflex), the relative-`choice` collapse to `none`, and the per-question forward-pass multiplier.
- **Sentence-level classification is the span mechanism.** The claim is verbatim by construction.
- **Long inputs:**
  - Qwen3.5-4B handles 8k tokens natively (262k context, [card](https://huggingface.co/Qwen/Qwen3.5-4B)), but *trained* on long drafts, since the clones' short-state training does not transfer. Kev: "Contexts beyond training length are untrained" ([repo](https://github.com/jaredpalmer/kev)). reflex's adapters lost judgement "on long, ambiguous inputs" ([repo](https://github.com/kshetrajna12/reflex)).
  - For ModernBERT (8,192 tokens), chunk with overlap, as GLiNER2 does with 384-word chunks and 64 overlap ([repo](https://github.com/fastino-ai/GLiNER2)).
  - `[INFERRED]` The contradiction tell (RTD-8/9/15) needs two distant sentences. Chunking breaks it, so keep that rule whole-draft or on Haiku.
- **Two-stage cascade with Haiku as the escalation tier** `[INFERRED]`:
  - Local model at a **high-recall threshold** per rule, then send only the flagged (rule, top-k sentences) pairs to the existing Haiku per-rule judge with the quote constrained to the candidates. This cuts Haiku calls from 22 per draft to the handful flagged.
  - Directly attacks the preregistered failure prediction (22 independent calls → ~49% of clean texts fire at a 3% per-rule rate; amendment at `rule-injection-timing-preregistration.md:286ff`).
  - Rules with too little data bypass stage 1 and always go to Haiku.
  - This matches rung 2's rule that "every judgement site must declare its behaviour when the judge is unavailable, slow, or low-confidence" (tracker `d16552e9981f521e` § The ladder).
- **Ensembling.** Averaging two option orders "roughly halves external ECE" for letter readout ([reflex](https://github.com/kshetrajna12/reflex)). With sentence heads there is no option order. `[INFERRED]` The cheap ensemble is ModernBERT + Qwen head agreement as a precision filter.
- **Active learning from phase-2 outcomes** `[INFERRED]`:
  - Every Score-B-style fork yields (draft, injected (rule, claim), did the violation stop).
  - A stopped violation is weak evidence that the binding was valid. A non-stop is ambiguous.
  - Queue uncertainty-band items (0.3–0.7) and local-versus-Haiku disagreements for **human** adjudication, never self-labelling.
  - The design tracker is explicit: "No automatic online fine-tuning should convert the worker's own completion claim into truth" (§ Learning material and evaluation).
  - Retrain in batches against a frozen eval, never on it.

## 6. Recommended path

Each step is registered before its model calls run, per the standing constraints.

**Step 0: policy decision (blocking for Steps 3–4, not for 1–2).** Decide under the AUP/Consumer-Terms clauses above whether Claude-authored drafts may be training inputs. Record the ruling in the preregistration. Effort: operator time.

**Step 1: zero-shot local arms, no training, about 1–2 days of engineering.** Results decide whether the backbone has any signal on our distribution.
- **L0-frozen:** JevK5 (or frozen Qwen3.5-4B via SemIf's readout) served locally on `/v1/systemone`, with per-rule `noul` plus windowed sentence `choice` (≤16 per window), exactly the configuration in the previous report §8, but local. It reuses `jev_select`'s wire shape, since `typesafe`-shaped servers are drop-in.
- **L0-embed:** MiniLM sentence embeddings + nearest-rule similarity. This is the tracker's rung 3, with no training yet.
- Score on the gate cases and Score A.
- **Kill criterion** `[proposal]`: if L0-frozen does not beat a trivial prior on the gate, it confirms the sealed-set finding for our domain, and the path goes straight to Step 3 or stops.

**Step 2: data build, about 3–6 person-days `[INFERRED]`.**
- Mine the repo history for correction pairs.
- Synthesise contrastive pairs per rule with a local open-weight teacher: `[INFERRED]` Qwen3.6-27B at ~4-bit is ≈14–16 GB and fits the A5000 for offline generation.
- Human spot-check, run the dedup and overlap filters, and freeze and hash the eval sets.
- Target ≥50 positive sentences per trained rule, plus hard negatives. Rules below that stay Haiku-only.

**Step 3: trained heads, 1–3 days plus overnight runs.**
- **L1-MBERT:** ModernBERT-large + sentence × rule head, 8k-token chunks.
- **L2-QWEN:** Qwen3.5-4B (init from JevK5) + LoRA r16 + sentence × rule head, whole draft, BCE, per-rule temperature on the calibration fold.

**Step 4: cascade.** **C1** = best of L1/L2 at a recall-oriented threshold, then Haiku verifies only the flagged pairs.

**Arms to register** (the same route and judge channel as the `e2s` arm):

| arm | stage 1 | stage 2 | purpose |
|---|---|---|---|
| **H0** | — | Haiku per-rule judge × 22 (current `phase1-span-selector.py`) | baseline |
| L0-frozen | JevK5/Qwen3.5-4B zero-shot | — | is there base signal? |
| L0-embed | MiniLM similarity | — | rung-3 floor |
| L1-MBERT | fine-tuned ModernBERT head | — | cheap trained arm |
| L2-QWEN | fine-tuned Qwen3.5-4B head | — | main trained arm |
| **C1** | best local, high recall | Haiku on flagged pairs only | the deployable candidate |

**Metrics:**
- **Score A** (authored gold, per `text_detectable` bucket), exactly as preregistered:
  - recall on yes+partial positives;
  - "gold only" rate;
  - any-fire rate on negatives;
  - **plus span exact-match** (chosen sentence contains the gold claim).
  - Report Wilson intervals. `[INFERRED]` At n=21, 10/21 has a 95% CI of about [0.28, 0.68], so Score A can only separate large effects.
- **Score B** (end-to-end fork effect): arm 0 − arm ≥ 0.4 per rule, and arm − 1b ≤ 0.2, against H0's `e2s`.
- **Cost:** Haiku tokens per draft (C1 versus H0), local latency per draft, and per-rule ECE/Brier on the held-out calibration fold.
- **Ship rule** `[proposal]`: C1 ships if it is non-inferior to H0 on Score B within the preregistered margins **and** cuts Haiku calls per draft by ≥50%. L1 or L2 alone ship only if they match H0 on Score B, which I do not expect given the sealed-set evidence.

**Cost summary** `[INFERRED]`: no cash outlay (local GPU, open weights, subscription for evaluation only). Engineering effort is about 1.5–3 weeks end to end, dominated by the data build. The risk-adjusted most likely outcome is C1: a local prefilter that cuts Haiku spend and false positives, with Haiku kept as the judge of record.

## Key risks

1. **Generalisation.** On novel hard items every Jev-class system sits near chance (sealed accuracy 26–37% against 29.3%). Small data and a narrow domain make a public-versus-real gap likely, the same pattern as the benchmark's.
2. **Policy.** The AUP distillation clause may bar training on Claude-authored text as well as Claude labels. This is unresolved and blocking.
3. **Data scarcity per rule.** Several rules have 1–4 cases, and some will never be trainable.
4. **Long-input degradation.** reflex rejected all its adapters for losing judgement on long, ambiguous inputs.
5. **Sentence granularity versus phase 2's hand-trimmed claims.** It is untested whether whole-sentence claims keep the 0/9–0/10 effect.
6. **Calibration does not transfer across distributions,** so per-rule refits are needed.
7. **Eval reuse.** Score A has 21 pairs; repeated gating will overfit it.

## Not found

- Winnow-12B's licence, weights location and training recipe (the corpus is private).
- Hopper's training hyperparameters, data sizes and VRAM figure.
- JevK5's training hardware and time, LoRA alpha and batch size, and its trained sequence length (only the 16,384-token serving cap).
- A context-length limit for SemIf, Hopper, reflex and djev.
- djev's VRAM requirement.
- A JevBench procedure for contributing tasks.
- Full per-system licences on the leaderboard (the page says "check the licence").
- v1.4 sealed and public numbers for Jobe, Kev, Nimble, Jev-Omni and local-jev (the results JSON fetch was truncated).
- A method in the Kovács et al. abstract for non-verbatim generated spans.
- Any clone that outputs a free-text span. None of the JevBench entrants emits text; GLiNER2 extracts spans but is not trained for rule judgments.
- A current (2026) US-region Consumer Terms page. The fetched page was the EEA version, effective 2025-10-08; re-check the live US terms before relying on them.

## Sources

- JevBench leaderboard and method: https://benchmarkheaven.com/jev-models ; https://github.com/fstandhartinger/jevbench ; https://github.com/fstandhartinger/jevbench/blob/main/docs/METHOD-v1.4.md ; https://raw.githubusercontent.com/fstandhartinger/jevbench/main/results/v1.4/jevbench-v1.4-results.json
- JevK5: https://github.com/allebee/jevk5 ; https://github.com/fstandhartinger/jevbench/issues/31 ; https://huggingface.co/alibiserikbay/JevK5
- Hopper: https://huggingface.co/HopitAI/hopper ; https://github.com/hopit-ai/hopper
- reflex: https://github.com/kshetrajna12/reflex ; https://huggingface.co/kshetrajna12/reflex-qwen3.5-4b-lora
- SemIf: https://github.com/TheoLeeCJ/SemIf
- djev: https://github.com/Davipar/djev-dev ; https://deepmind.google/models/gemma/diffusiongemma/
- Kev: https://github.com/jaredpalmer/kev
- Nimble: https://github.com/bespokelabsai/nimble ; https://jevainews.com/news/bespoke-nimble/
- Winnow: https://github.com/EldanRing/winnow-inference/issues/1
- Clone roundup: https://www.latent.space/p/ainews-here-are-6-clones-of-jev-in
- Jev architecture essay: https://archerhume.com/posts/jevs-architecture-unmasked/
- DecisionBench: https://github.com/Hanno-Labs/decision-bench
- Qwen3.5-4B: https://huggingface.co/Qwen/Qwen3.5-4B ; Unsloth: https://unsloth.ai/docs/models/qwen3.5/fine-tune
- ModernBERT: https://huggingface.co/answerdotai/ModernBERT-large ; GLiNER2: https://github.com/fastino-ai/GLiNER2
- Span detection: https://arxiv.org/abs/2502.17125 (LettuceDetect) ; https://arxiv.org/abs/2607.00895 (Kovács et al. 2026)
- Calibration: https://arxiv.org/abs/2308.13111 (Laplace-LoRA) ; https://proceedings.mlr.press/v70/guo17a.html
- Anthropic terms: https://www.anthropic.com/legal/aup ; https://www.anthropic.com/legal/consumer-terms ; https://www.anthropic.com/legal/commercial-terms
