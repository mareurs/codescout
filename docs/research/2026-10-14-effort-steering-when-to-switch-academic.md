> **Provenance (2026-10-14).** Raw report of a research subagent (Opus), copied unchanged from the session scratchpad. It is model output: only the items listed as verified in the index `docs/research/2026-10-14-effort-steering-when-to-switch.md` were re-checked at the source by the main session. Treat every other claim as a lead.

# Effort steering inside an agent trajectory: academic literature (2024-2026)

Compiled 2026-10-14 for the effort-routing work (Sonnet 5.5, Claude Code 2.1.296, `turn.step` mod, per-request effort router).
Scope: academic and research literature. One vendor page is included in section 1.6 because it bears directly on the mechanism.

**How to read the read-levels.**
- `read: abstract`: I opened the arXiv abstract page.
- `read: full`: I fetched the full HTML and asked targeted questions about named sections. This went through a fetch tool that summarises with a small model, so a number marked "full" was read through that summariser. For ARES I cross-checked the numbers with a second fetch.
- `snippet only`: the number came from a search-engine snippet, not from a page I read. The item's abstract page was still opened unless stated otherwise.

Every item below has a URL that I opened. Leads I saw but did not open are listed in section 3, and nothing is cited from them as evidence.

---

## 1. Findings per topic

### 1.1 Adaptive test-time compute and reasoning budgets (mostly per query)

- **Scaling LLM Test-Time Compute Optimally can be More Effective than Scaling Model Parameters** (Snell, Lee, Xu, Kumar). https://arxiv.org/abs/2408.03314. 2024-08-06. read: abstract.
  - Measured: how well a test-time method works (search against a process reward model, or sequential revision) depends strongly on how hard the prompt is. Allocating compute per prompt by difficulty was more than 4x as efficient as best-of-N.
  - Measured: at matched FLOPs, a small model with test-time compute beat a 14x larger model, but only on prompts where the small model already had "somewhat non-trivial" success.
  - Takeaway: what you allocate compute on is the prompt's difficulty for this model.

- **Learning How Hard to Think: Input-Adaptive Allocation of LM Computation** (Damani, Shenfeld, Peng, Bobu, Andreas). https://arxiv.org/abs/2410.04707. 2024-10-07 (ICLR 2025). read: abstract.
  - Method: predicts, per input, the reward distribution under each compute budget, then spends compute where the predicted *marginal* gain is largest. Used for adaptive best-of-k and for routing between an expensive and a cheap decoding procedure.
  - Measured on code, math and dialog: up to 50% less compute at no quality loss, or up to +10% quality at a fixed budget.
  - This is the clearest statement that a router should predict the *gain* from extra compute, not difficulty.

- **Rational Metareasoning for Large Language Models** (De Sabbata, Sumers, AlKhamissi, Bosselut, Griffiths). https://arxiv.org/abs/2410.05563. 2024-10-07 (v3 2025-06-23). read: abstract.
  - Method: a value-of-computation reward that penalises unneeded reasoning, used for expert iteration.
  - Measured: 20-37% fewer tokens across three models, with task performance held, against few-shot CoT and STaR.
  - Per query, not per agent step.

- **Do NOT Think That Much for 2+3=? On the Overthinking of o1-Like LLMs** (Chen et al.). https://arxiv.org/abs/2412.21187. 2024-12-30. read: abstract.
  - Defines outcome-side and process-side efficiency metrics for overthinking. A self-training method shortens reasoning while keeping accuracy on GSM8K, MATH500, GPQA and AIME.
  - The abstract gives no numbers.

- **Thoughts Are All Over the Place: On the Underthinking of o1-Like LLMs** (Wang et al.). https://arxiv.org/abs/2501.18585. 2025-01-30. read: abstract.
  - Measured (3 test sets, 2 models): frequent switching between lines of thought correlates with wrong answers.
  - A decoding penalty on thought switching (TIP) raises accuracy with no fine-tuning.
  - Takeaway: too little depth is a failure mode separate from too much.

- **s1: Simple test-time scaling** (Muennighoff et al.). https://arxiv.org/abs/2501.19393. 2025-01-31. read: abstract.
  - "Budget forcing": stop thinking early, or extend it by appending "Wait". Extending made the model re-check its work.
  - Measured: AIME24 rose from 50% to 57% for s1-32B.

- **L1: Controlling How Long A Reasoning Model Thinks With RL** (Aggarwal, Welleck). https://arxiv.org/abs/2503.04697. 2025-03-06 (COLM 2025). read: abstract.
  - LCPO trains a model to obey a length budget given in the prompt, giving a smooth cost/accuracy trade-off. Claimed to beat s1's length control.

- **Qwen3 Technical Report**. https://arxiv.org/abs/2505.09388. 2025-05-14. read: abstract.
  - One model with a thinking mode and a non-thinking mode, plus a user-set "thinking budget". This is the hybrid-mode design that per-request effort control builds on.

- **AdaptThink: Reasoning Models Can Learn When to Think** (Zhang et al.). https://arxiv.org/abs/2505.13417. 2025-05-19. read: abstract.
  - Measured: on easy problems, NoThinking beats Thinking on both accuracy and cost.
  - RL that picks the mode per problem: on DeepSeek-R1-Distill-Qwen-1.5B, average length fell 53% and accuracy rose 2.4% over three math sets.

- **Thinkless: LLM Learns When to Think** (Fang, Ma, Wang). https://arxiv.org/abs/2505.13379. 2025-05-19. read: abstract.
  - Uses `<short>`/`<think>` control tokens. Plain GRPO collapsed onto one mode; the decoupled loss (DeGRPO) fixed this. Long-chain use fell 50-90%.
  - Relevant to us: learned mode selectors tend to collapse onto one mode.

- **Learning When to Think: Adaptive Reasoning for Test-Time Compute Allocation** (Kassenaar, Yang, François-Lavet). https://arxiv.org/abs/2608.20256. 2026-08-20. read: abstract.
  - The model picks NoThink, Short or Long with its first token.
  - Measured: MATH500 accuracy 0.782 vs 0.796 for the base model, mean length 4,796 → 2,811 tokens (-41%), GSM8K -76% tokens.

- **Inverse Scaling in Test-Time Compute** (Gema et al., Anthropic Fellows). https://arxiv.org/abs/2507.14417. 2025-07-19 (TMLR 12/2025). read: abstract.
  - Measured: on four constructed task families, longer reasoning lowers accuracy. Claude models in particular become more distracted by irrelevant information.
  - The authors recommend evaluating across a range of reasoning lengths.

- **Reasoning on a Budget: A Survey of Adaptive and Controllable Test-Time Compute** (Alomrani et al.). https://arxiv.org/abs/2507.02076. 2025-07-02. read: abstract.
  - Taxonomy: L1 "controllability" (fixed budgets) and L2 "adaptiveness" (adjusts to difficulty or confidence). Useful as a map; no step-level agent results.

- **Holistic Agent Leaderboard (HAL)** (Kapoor et al.). https://arxiv.org/abs/2510.11977. 2025-10-13. read: abstract; the 21/36 figure is snippet only.
  - Scale: 21,730 rollouts, 9 models, 9 benchmarks, about $40k.
  - Abstract: "higher reasoning effort reducing accuracy in the majority of runs".
  - Snippet only: higher effort did not improve accuracy in 21 of 36 model/scaffold/benchmark runs, and Anthropic "high" meant a LiteLLM default of 4,096 thinking tokens. That is not today's `effort` parameter.

- **Low Reasoning Effort Is Enough for Routine Office Work by Language-Model Agents** (Xu, Wu). https://arxiv.org/abs/2608.03169. 2026-08-04 (v2 2026-09-29). read: abstract.
  - 840 runs, two GPT-5.6 tiers, 14 tasks.
  - Measured: at both low and max effort the agent always obeyed the rules. Low used about 43% fewer output tokens and about 20% less time.
  - The outcome measured is rule compliance, not success on hard tasks.

### 1.2 Step-level compute allocation inside multi-step agents

#### (a) Methods that choose effort or thinking per step

- **Ares: Adaptive Reasoning Effort Selection for Efficient LLM Agents** (Yang, Hou, Wei, Bao, Chang). https://arxiv.org/abs/2603.07915. 2026-03-09. read: full (HTML; Table 1 re-fetched to confirm). This is the closest published analogue of our router.
  - **Setup.** The agent is gpt-oss-20b with low/medium/high effort. The router is Qwen3-1.7B. It sees the task, the interaction history and the current observation, **not** the proposed action. It writes a short rationale, then a label.
  - **Labels.** The paper runs a high-effort reference trajectory, freezes its prefix, and replays each step K=3 times per effort level. The label is the cheapest level whose samples reproduce the reference action (a GPT-4o judge decides semantic matches). Training is SFT, then GRPO on TAU-Bench only.
  - **Table 1 (accuracy % / total reasoning tokens):**

    | Benchmark | Low | Medium | High | Random | GPT-5 prompted | Gemini 3 Pro prompted | Ares |
    |---|---|---|---|---|---|---|---|
    | TAU Retail | 35.0 / 25k | 47.3 / 137k | 54.8 / 1007k | 43.5 / 359k | 40.0 / 132k | 46.1 / 128k | 54.8 / 652k |
    | TAU Airline | 32.0 / 12k | 42.0 / 98k | 38.0 / 873k | 34.0 / 521k | 34.0 / 396k | 36.0 / 239k | 36.0 / 678k |
    | BrowseComp+ | 8.0 / 5k | 34.0 / 538k | 42.7 / 1841k | 30.7 / 392k | 38.7 / 1398k | 37.3 / 1144k | 41.3 / 1071k |
    | WebArena | 37.4 / 67k | 42.6 / 538k | 45.0 / 2763k | 40.3 / 857k | 41.1 / 1159k | 41.9 / 1164k | 46.5 / 1512k |

    The RL version on Retail reached 58.5 / 476k, which is the source of the headline "up to 52.7%" saving.
  - **Measured observations.**
    - On Airline, fixed *medium* beat fixed *high*.
    - The prompted frontier-LLM choosers (GPT-5, Gemini 3 Pro) scored below fixed medium on 3 of 4 benchmarks.
    - The random baseline samples uniformly per step and is **not budget-matched**.
    - No seeds or variance are reported.
  - **Where high effort went.**
    - On WebArena, steps 0-2 were mostly low; high became more frequent later, which the authors attribute to more complex observations.
    - `go_back` and `branch` actions (recovery and re-planning) got the largest share of high effort.
    - The label mix depends on the task. TAU Retail labels were 24,875 low of 43,358 (57%); WebArena was 1,095 high of 1,718 (64%).
  - **Design note.** The authors switch effort within one model rather than switching models, so the KV cache is reused.

- **Think Fast and Slow: Step-Level Cognitive Depth Adaptation for LLM Agents (CogRouter)** (Yang et al.). https://arxiv.org/abs/2602.12662. 2026-02-13. read: abstract.
  - Four ACT-R-inspired cognitive levels, chosen per step. Training is SFT, then CoPO, which assigns credit per step using *action confidence*.
  - Measured on ALFWorld and ScienceWorld (Qwen2.5-7B): 82.3% success, 62% fewer tokens.
  - Snippet only: plain GRPO collapsed to the deepest level throughout the trajectory.

- **Not All Turns Are Equally Hard: Adaptive Thinking Budgets for Efficient Multi-Turn Reasoning in Agents (TAB)** (Jali, Nayak, Joshi). https://arxiv.org/abs/2604.05164. 2026-04-06 (v3 2026-08-08). read: abstract.
  - Treats per-turn budgets as a multi-objective MDP; the policy is trained with GRPO under a global token budget.
  - Measured: up to 35% fewer tokens and 30% lower latency. When the plan of sub-questions is known in advance, up to 40% fewer tokens.

- **Adaptive Latent Agentic Reasoning (ALAR)** (Jung et al.). https://arxiv.org/abs/2606.02871. 2026-06-01. read: abstract.
  - Claim: agents "allocate reasoning effort nearly uniformly across turns".
  - Measured: up to -43.6% tokens on search and -84.6% on tool use, at comparable or better accuracy.
  - Snippet only, not opened: a GitHub critique says ALAR never tests per-turn difficulty, so the evidence supports a policy conditioned on task and position, not per-turn targeting.

- **When Should Agents Think? Adaptive Reasoning via Cross-Turn Estimation (RACE)** (Cheng et al.). https://arxiv.org/abs/2610.12061. 2026-10-08. read: abstract.
  - Measured: the drop in likelihood of later reference actions when a turn's reasoning is removed tracks whether those actions are still supported by earlier reasoning. Reasoning written in one turn often supports several later turns.
  - Claim: fresh reasoning is needed only when the context changes enough to make the old reasoning insufficient.
  - Four benchmarks; the abstract gives no numbers.
  - Snippet only: baselines included Anthropic's "think tool", DART and AdaptThink.

- **Geometry of Divergence: Tracking Hidden-State Trajectories for Adaptive Multi-Turn Reasoning** (Liang et al.). https://arxiv.org/abs/2608.30650. 2026-08-31. read: abstract.
  - Measured: hidden-state curvature and variance slope separate correct from incorrect episodes before they end. How well they separate depends on the action-chain pattern (Read/Write/Respond/Transfer).
  - Measured: triggering thinking from these signals raised τ-Bench success from 24.1% to 39.6% at -11.2% tokens.
  - Snippet only, not opened: an automated reviewer says thresholds were selected on the same episodes that were scored.

- **Learning When to Plan: Efficiently Allocating Test-Time Compute for LLM Agents** (Paglieri et al.). https://arxiv.org/abs/2509.03581. 2025-09-03 (v3 2026-02-17). read: abstract.
  - Measured in Crafter: planning at every step is costly *and* hurts long-horizon performance; never planning is worse still.
  - SFT followed by RL teaches the agent when to plan.

- **Scaling Test-time Compute for LLM Agents** (Zhu et al.). https://arxiv.org/abs/2506.12928. 2025-06-15. read: full (reflection section).
  - Setup: GAIA validation (165 tasks), SmolAgents with GPT-4.1.
  - Measured overall success: baseline 55.76, reflect every step 55.15, reflect when the verifier score is <8 53.33, <5 52.12, <2 56.36.
  - The authors conclude that "knowing when to reflect" matters. The gain is +0.6 points with no variance reported, so it is weak evidence.

- **The Danger of Overthinking: Examining the Reasoning-Action Dilemma in Agentic Tasks** (Cuadron et al.). https://arxiv.org/abs/2502.08235. 2025-02-12. read: abstract.
  - 4,018 SWE-bench Verified trajectories (CodeAct scaffold). Three patterns: Analysis Paralysis, Rogue Actions and **Premature Disengagement**.
  - Measured: higher overthinking scores went with lower resolve rates; reasoning models overthought more.
  - Measured: picking the lower-overthinking of k samples improved performance by almost 30% and cut cost by 43%.

- **AgentTTS: LLM Agent for Test-time Compute-optimal Scaling in Complex Tasks** (Wang et al.). https://arxiv.org/abs/2508.00890. 2025-07-26 (NeurIPS 2025). read: abstract.
  - Allocates compute across the stages of multi-stage tasks. Insights from pilots on 4 tasks / 6 datasets.
  - Snippet only: the compute given to earlier subtasks changes how later subtasks scale, so per-stage allocations interact.

- **Budget-Aware Tool-Use Enables Effective Agent Scaling (BATS)** (Liu et al., Google). https://arxiv.org/abs/2511.17006. 2025-11-21 (COLM 2026). read: abstract.
  - Measured: a bigger tool-call budget alone hits a ceiling. Telling the agent its remaining budget (Budget Tracker) and adapting planning and verification to it moves the cost/performance Pareto frontier.
  - The abstract gives no numbers.

- **Selective Critique for Cost-Aware LLM Agents in Long-Horizon Decision Making (SAG)** (Park, Im, Kwon). https://arxiv.org/abs/2610.07335. 2026-10-05 (NeurIPS 2026). read: abstract.
  - A gate calls the critic only when its estimated value of information exceeds its cost. The gate uses entropy and the top-2 margin over allowed actions.
  - Measured on ALFWorld: success 24.6% → 78.4% at token spend similar to ReAct.

- **Do AI Agents Know When a Task Is Simple? (E3)** (Yin, Feng). https://arxiv.org/abs/2607.13034. 2026-07-14. read: abstract.
  - "Estimate, Execute, Expand": run the minimal path, and widen scope only if verification fails.
  - Measured in a 121-edit simulator: -85% cost at 100% success. A small gpt-4o live case showed comparable success.

- **Don't Overthink, Don't Underthink: Toward Adaptive Reasoning in Agentic AI** (Mia, Amini). https://arxiv.org/abs/2608.26442. 2026-08-26. read: abstract.
  - On MATH-500 and GAIA: over-reasoning goes with higher cost "without proportional accuracy gains"; under-reasoning goes with wrong or incomplete solutions. No numbers in the abstract.

#### (b) Where coding agents fail: the end of the run (verification and the stop decision)

- **Coherence Collapse: Diagnosing Why Code Agents Fail After Reaching the Right Code** (Kim et al.). https://arxiv.org/abs/2603.24631. 2026-03-25 (v2 2026-05-26). read: abstract.
  - 16,758 trajectories, 3 scaffolds, 7 models.
  - Measured: on SWE-Agent and OpenHands, 60-69% of failures find and modify the right functions but still produce a wrong patch. The largest theme is that the agent reaches correct code and then overwrites or churns it.
  - In 5 cases the agent's mid-run patch was bit-identical to gold and was later destroyed; an edit-commit checkpoint recovered all 5.
  - A consensus variant gave +3.0 pp on GPT-5 (p=0.08).

- **SWE-Marathon: Can Agents Autonomously Complete Ultra-Long-Horizon Software Work?** (Desai et al.). https://arxiv.org/abs/2606.07682. 2026-06-05. read: abstract; percentages snippet only.
  - Measured: frontier agents solve under 30% of 20 long tasks. Named failure causes are poor self-verification, self-reported infeasibility and **premature termination**. Reward hacking appeared in 13.8% of rollouts.
  - Snippet only: premature termination was 7.6% and poor self-verification 4.0% of failures; one agent submitted with 778 of 780 visible tests passing.

- **Building to the Test: Coding Agents Deliver What You Check, Not What You Requested** (Ma, Kereopa-Yorke, Schultz). https://arxiv.org/abs/2606.28430. 2026-06-26. read: abstract.
  - 18 runs with claude-opus-4.7 and gpt-5.5 in Copilot CLI.
  - Coins "validation self-awareness": whether the agent, unprompted, starts the validation a user would run. Measured in that small set: when an oracle was in the loop, scores were near perfect while the requested library was dead or missing.

- **Rethinking the Value of Agent-Generated Tests for LLM-Based SWE Agents** (Chen et al.). https://arxiv.org/abs/2602.07900. 2026-02-08 (v2 2026-04-09). read: abstract.
  - Six LLMs on SWE-bench Verified. Measured: within a model, solved and unsolved tasks show similar test-writing rates.
  - A prompt intervention on 4 models that pushed test writing up or down did not significantly change outcomes.
  - Caution for us: prompting for *more testing* may change cost, not outcomes. Note this concerns writing *new* tests, not running the existing full suite.

- **Beyond Resolution Rates: Behavioral Drivers of Coding Agent Success and Failure** (Mehtiyev, Assunção). https://arxiv.org/abs/2604.02547. 2026-04-02. read: abstract.
  - 9,374 trajectories, 19 agents, 500 tasks.
  - Measured: the "longer trajectory means failure" association **reverses once task difficulty is controlled**.
  - Gathering context before editing, and spending effort on validation, predict success. The agent sets these strategies itself and does not adapt them to the task.

- **When Agents go Astray: Course-Correcting SWE Agents with PRMs (SWE-PRM)** (Gandhi et al., IBM). https://arxiv.org/abs/2509.02360. 2025-09-02 (v2 2025-10-21). read: abstract.
  - The taxonomy of inefficiencies includes "failure to terminate once a solution is reached".
  - Measured: an inference-time PRM giving taxonomy-guided feedback raised SWE-bench Verified from 40.0% to 50.6%, at an extra cost as low as $0.2.

- **Fail-Fast, Restart-Smart: Early Failure Prediction and Restart for SWE Agentic Tasks** (Wang et al., Lo). https://arxiv.org/abs/2608.03222. 2026-08-04. read: abstract.
  - A 0.6B monitor predicts failure from the *observable trajectory prefix* (no logits or hidden states).
  - Measured: 14.6-20.4% of execution tokens saved at a 5% false-positive target, transferring to 3 other policies including a closed API model. Restart raised Qwen3.6-27B from 66.6% to 71.8%.

- **TACT: Mitigating Overthinking and Overacting in Coding Agents via Activation Steering** (Sui et al.). https://arxiv.org/abs/2605.05980. 2026-05-07. read: abstract.
  - Measured: step labels (overthinking / overacting / calibrated) are linearly separable in hidden states, AUC about 0.9.
  - Steering raised resolve rate by +5.8 pp on Qwen3.5-27B and +4.8 pp on Gemma-4-26B, with up to 26% fewer steps.
  - Needs model internals.

- **Thinking Longer, Not Larger: Enhancing SWE Agents via Scaling Test-Time Compute** (Ma et al.). https://arxiv.org/abs/2503.23803. 2025-03-31. read: abstract.
  - Measured: a 32B model reaches 46% on SWE-bench Verified and spends more tokens on harder problems.
  - The abstract claims "targeted computational allocation at critical development decision points" but **does not name which stages**. The researcher tool's summary said "fault localization and patch generation"; the abstract does not support that, so I do not rely on it.

- **Coding Agents Don't Know When to Act** (Gloaguen et al., ETH). https://arxiv.org/abs/2605.07769. 2026-05-08. read: abstract; effort numbers snippet only.
  - FixedBench: 200 tasks where no change is needed; agents still made unwanted changes in 35-65% of cases.
  - Snippet only: varying effort over four levels moved correct-abstention from 61.5% to 65.8%, inside about ±7% CIs, which the authors call no significant effect. They argue the fix is task framing, not compute.

- **Ask Early, Ask Late, Ask Right: When Does Clarification Timing Matter for Long-Horizon Agents?** (Gulati et al.). https://arxiv.org/abs/2605.07937. 2026-05-08. read: abstract.
  - A forced-injection framework supplies correct clarifications at chosen points (3 benchmarks, 4 models, more than 6,000 runs).
  - Measured: goal clarification is worth almost nothing after about 10% of execution; input clarification stays useful until about halfway. **Clarifying after the midpoint left performance worse than never asking.**
  - In 300 unscripted sessions, no model asked inside the best window.
  - Relevant to our cs-p12 loss, where the agent stopped at the end to ask.

- **Agentic Abstention: Do Agents Know When to Stop Instead of Act?** (Luo, Wen, Wang). https://arxiv.org/abs/2606.28733. 2026-06-27. read: abstract.
  - 13 systems, more than 28,000 tasks. Measured: when an agent stops matters as much as whether it stops. Model scale and reasoning affect timely abstention in different directions.
  - The abstract has no per-effort breakdown.

#### (c) Whole-run effort sweeps on coding agents (closest to our setup)

- **Prompt-Induced Waste in Coding Agents: Reasoning, Effort, Harness Design, and End-to-End Cost** (Weinberger, Hozez, PointFive). https://arxiv.org/abs/2608.01347. Version read: v6, dated 2026-09-10; the v1 date was not visible. read: full (HTML fetched in two parts; Appendices M and N).
  - **SWE-Effort campaign (Appendix N).**
    - Tasks: 25 SWE-bench Verified instances whose gold patches touch at least 2 files and 40 lines. Stage gate: low-effort success had to fall between 30% and 80%.
    - Setup: Claude Code 2.1.220 at low, high and xhigh; Sonnet 5 and Opus 5; 150 cells paired by task.
    - Resolved (cost per resolved task):

      | Model | low | high | xhigh |
      |---|---|---|---|
      | Opus 5 | 17/25 ($0.75) | 18/25 ($1.89) | 16/25 ($3.62) |
      | Sonnet 5 | 12/25 ($1.76) | 19/25 ($2.32) | 17/25 ($3.00) |

    - Paired McNemar tests:
      - Sonnet xhigh vs low: 6 rescues, 1 regression, p=0.125, not significant after Holm.
      - Sonnet high vs low: 7 rescues, 0 regressions, p=0.016.
      - Opus xhigh vs low: p=1.0.
    - **Only 3 of 6 Sonnet xhigh rescues reproduced on rerun, and 6 of 10 discordant tasks were solved by both levels on rerun.**
    - The reproduced rescues fixed "cross-module-completeness failures" of the low-effort run.
    - All 6 timeouts and all 4 empty patches happened at high or xhigh. Opus's xhigh regressions were a scope-expanding edit that broke passing tests, and a timeout.
    - A simulated escalation rule fired on 64-68% of tasks and "did not improve the observed cost–success frontier".
    - Authors' conclusions: for Sonnet, xhigh is dominated by high; for Opus, low is on the frontier.
  - **Adaptive-Compute campaign (Appendix M).**
    - 8 purpose-built hard tasks, 320 runs. At low, both models solved all 8, so no adaptive arm could be tested (the preregistered gate needed baseline success below 90%).
    - Static max effort raised cost per success 1.5-2.8x, gained no successes and lost one.
    - Escalation rarely fired because the first phase's visible tests passed.
    - The only Sonnet failures came from "stop early" wording on an ambiguous-spec task: visible tests passed, hidden tests failed, and the requirements file was never read.
  - The core benchmark was preregistered. Whether the effort campaigns were preregistered was not visible in what I read.

- **Confucius Code Agent** (Wong et al., Meta and Harvard). https://arxiv.org/abs/2512.10398. 2025-12-11 (v6 2026-02-03). read: full (Appendix D only).
  - Claude 4 Sonnet with `thinkingBudget` at 8k / 16k / 32k: Pass@1 of 67.3 / 68.4 / 68.7 on an unstated subset of SWE-bench Verified.
  - The authors note the budget is only a cap and does not control actual thinking length.

- **Can your AI agent be cheaper? Effects of task specifications on token spend in agentic coding tasks** (Smékal). https://arxiv.org/abs/2608.25399. 2026-08-26. read: abstract; effort multipliers snippet only.
  - 2,700 runs with Kimi K3 at low, high and max. Measured: a bare user story raised spend 29.7% over a full spec. A predictor using one cheap probe run is accurate to within 36%.
  - Snippet only: low → max raised cost x3.62 and x4.35 on two tasks that were solved almost every time.

- **PostTrainBench** (Rank et al.). https://arxiv.org/abs/2603.08640. 2026-03-09. read: abstract; the effort claim is snippet only and not verified.
  - Snippet: for GPT-5.1 Codex-Max, default medium effort did best; high used about 2x the tokens, which the authors blame on extra context compaction.

### 1.3 Model routing and cascades: from per query to per step

**Per query (foundations and metrics):**

- **FrugalGPT** (Chen, Zaharia, Zou). https://arxiv.org/abs/2305.05176. 2023-05-09 (older than 2024, but foundational). read: abstract.
  - An LLM cascade matched the best single model at up to 98% lower cost, or gave +4% accuracy at equal cost.

- **RouterBench** (Hu et al., Martian). https://arxiv.org/abs/2403.12031. 2024-03-18. read: full (AIQ and baselines).
  - 405k inference outcomes.
  - **AIQ** is the area under the router's non-decreasing convex hull in the cost/quality plane, normalised by the cost range. Mixing two routers with probability t reaches any point on the segment between them.
  - Baselines: the **Oracle** (cheapest best model per query), and the **Zero router**, a probabilistic mixture of the fixed models on their joint convex hull. A router "is meaningful only if" it beats the zero router.

- **Hybrid LLM: Cost-Efficient and Quality-Aware Query Routing** (Ding et al.). https://arxiv.org/abs/2404.14618. 2024-04-22 (ICLR 2024). read: abstract.
  - Routes on predicted difficulty with a quality threshold that can be tuned at test time. Up to 40% fewer large-model calls with no quality drop.

- **RouteLLM: Learning to Route LLMs with Preference Data** (Ong et al.). https://arxiv.org/abs/2406.18665. 2024-06-26 (v4 2025-02-23). read: full (metrics).
  - **PGR** = (r_router − r_weak) / (r_strong − r_weak).
  - **APGR**: PGR averaged over strong-call fractions from 0 to 1 (10 points).
  - **CPT(x%)**: the minimum share of strong calls needed to reach x% PGR.
  - Baseline: a random router *under the same cost constraint*, which gives APGR ≈ 0.5 and CPT(50%) ≈ 50%.
  - Measured: more than 2x cost reduction. On MT-Bench, 3.66x cheaper than GPT-4 at CPT(50%) (95% of GPT-4 quality).

- **Self-Route: Automatic Mode Switching via Capability Estimation** (He et al.). https://arxiv.org/abs/2505.20664. 2025-05-27. read: abstract.
  - A router reads capability-aware embeddings from the model's hidden layers to choose short or long CoT. 30-55% fewer tokens at matching accuracy.

- **RADAR: Reasoning-Ability and Difficulty-Aware Routing for Reasoning LLMs** (Fernandez et al.). https://arxiv.org/abs/2509.25426. 2025-09-29 (ICLR 2026). read: abstract; numbers snippet only.
  - Fits an item-response model with interpretable query difficulty and per-(model, budget) ability; routes across model × reasoning-budget pairs. 8 benchmarks.
  - Snippet only: 35 configurations, including o4-mini low/medium/high; 90% of o4-mini-high on MATH-500 at 1.31% of the cost.

- **STEER: Confidence-Guided Stepwise Model Routing for Cost-Efficient Reasoning** (Lee et al.). https://arxiv.org/abs/2511.06190. 2025-11-09. read: abstract.
  - Before each reasoning step, the small model's logit confidence decides whether to call the large model.
  - Measured on AIME: up to +20% accuracy with 48% fewer FLOPs than the large model alone.

**Within an agent trajectory (per step):**

- **AgentRouter: Heterogeneous Model Routing for Cost-Optimal Multi-Step Agentic Workflows** (Paul, Nandy). https://arxiv.org/abs/2609.22951. 2026-09-19 (ICML 2026 AgenticUQ workshop). read: abstract (the HTML page returned 404).
  - A 12M-parameter classifier using 5 routing-time features assigns each step to one of 4 tiers; trained on 50k annotated steps.
  - Measured: -72% cost at 97.3% of frontier-only quality. RouteLLM and FrugalGPT applied per step saved only 31% and 44%.
  - The abstract names neither the benchmarks nor the features.

- **Do I Need the Cloud? Uncertainty-Aware Step-Level Handoff for SLM Agents (STEPGATE)** (Younesi). https://arxiv.org/abs/2610.07816. 2026-10-06 (NeurIPS 2026 workshop). read: full (method section).
  - **Design.** The local model *proposes* an action, and a calibrated gate scores it into execute locally, verify/retry once, or escalate.
  - **Signals.**
    - Token uncertainty and tool-name margin (need logits).
    - Schema validity.
    - Agreement across K cheap samples.
    - State: trajectory length, previous tool errors, repeated actions, empty observations, retry budget.
    - Risk: whether the call is read-only or state-changing.
  - **Thresholds** were set on validation data to meet a 30% cloud budget fixed before testing.
  - **Random baseline.** In the multi-turn test, random escalation was tuned to the same 30.0% rate and run over 5 seeds. In the single-step table it was not matched (33.8% vs 30.8%); a matched appendix variant scored 74.7%.
  - **Multi-turn results (trajectory success):**
    - StepGate 69.0% at 30% cloud actions
    - rate-matched random 60.0%
    - query-level routing 57.0%
    - local only 48.0%
    - strong only 82.0%
  - The models are Qwen2.5-1.5B (local) and 7B (cloud), on scripted tasks with small test sets.

- **Step-level Optimization for Efficient Computer-use Agents** (Wei et al.). https://arxiv.org/abs/2604.27151. 2026-04-29. read: abstract.
  - A small policy runs by default. A **Stuck Monitor** (loops, repeated actions, no progress) escalates to the strong model, and a **Milestone Monitor** picks checkpoints where sparse verification by the strong model catches "silent semantic drift". No numbers in the abstract.

- **RSI-Router: Evolving Subtask-Level LLM Routing and Skills** (Li et al.). https://arxiv.org/abs/2609.34712. 2026-09-28. read: abstract.
  - Routes per subtask between DeepSeek-V4.1-Flash and Qwen3.5-9B. Measured: beat DeepSeek-only on 5 benchmarks at 48.3% of the cost.

- **Agentic Routing: The Harness-Native Data Flywheel (OpenSquilla)** (Liu et al.). https://arxiv.org/abs/2607.11399. 2026-07-13. read: abstract.
  - Step-level model selection conditioned on the full harness state. Each decision is logged with its outcome and cost as training data for later routers. No numbers in the abstract.

- **Agent-as-a-Router: Agentic Model Routing for Coding Tasks** (Zhou et al.). https://arxiv.org/abs/2606.22902. 2026-06-22 (v3 2026-06-26). read: abstract.
  - Diagnoses an "information deficit" in routers: adding performance statistics per task dimension to an LLM router gave a +15.3% relative gain.
  - Releases CodeRouterBench (about 10K tasks scored for 8 LLMs) and compares routers by regret.

- **The Replay Gap: Static Evaluation of Model Switching in LLM Agents Scores the Wrong World** (Gonuguntla). https://arxiv.org/abs/2608.08239. 2026-08-08 (COLM 2026). read: abstract. Methodology; see 1.5.

### 1.4 Predicting from the state whether more compute will change the outcome

- **Learning How Hard to Think** (see 1.1) predicts the *marginal reward* of more compute.
- **Rational Metareasoning** (see 1.1) uses value of computation as a training reward.

- **LLMs Encode Their Failures: Predicting Success from Pre-Generation Activations** (Lugoloobi et al.). https://arxiv.org/abs/2602.09924. 2026-02-10 (v4 2026-08-11, COLM 2026). read: abstract.
  - Linear probes on activations taken before generation predict *this model's* success on math and code, beating question length and TF-IDF.
  - Models encode a model-specific difficulty that differs from human difficulty, and the gap grows with extended reasoning.
  - Routing a pool of models on the probes cut cost by up to 70% on MATH.

- **Reasoning Models Know When They're Right: Probing Hidden States for Self-Verification** (Zhang et al., NYU). https://arxiv.org/abs/2504.05419. 2025-04-07. read: abstract.
  - A probe on hidden states is well calibrated about whether intermediate answers are correct, and predicts upcoming correctness. Using it to stop early cut tokens by 24% with no loss.

- **It's the Problem, Not the Path: Budget and Difficulty Confounds in LLM Reasoning Trajectories** (Bulut). https://arxiv.org/abs/2609.03436. 2026-09-03. read: abstract. **The key caveat for any state-based router.**
  - Measured: a difficulty proxy that never looks at the trace reaches AUROC 0.873 on 192K R1 generations, inside the published range for probes.
  - A reconstructed early-window probe has pooled AUROC 0.849, but **within a problem it is at chance (0.496 at t=4)**.
  - A restart control at a matched token budget found only 1 of 178 problem/model cells where the prefix itself was what limited the solve.
  - Conclusion: a pooled AUROC does not show per-attempt information. Use a question-only baseline or within-problem evaluation.

- **From Uncertainty to Action: Learning to Steer LLM Agents (VoS)** (Li et al.). https://arxiv.org/abs/2610.09115. 2026-10-06. read: abstract. **The closest study to "does intervening at this step change the outcome".**
  - Built a "stepwise outcome table" of about 82,000 counterfactual continuations: 1,864 trajectories, 3 benchmarks, 2 agents, 4 steering mechanisms applied at every non-terminal step and run to the end.
  - **Measured: uncertainty flags failing trajectories, but no single signal reliably finds the step where steering helps.**
  - VoS learns the value of steering per step from measured outcomes, with a "harm budget" that caps interference with runs that would succeed. It beat unmodified execution in 12 of 12 settings (+7.8 points on average) and the best uncertainty-triggered method in 11 of 12 (+2.9).
  - Ablations: training on measured outcomes and a tight harm budget are both essential.

- **Doomed from the Start: Early Abort of LLM Agent Episodes via a Recall-Controlled Probe Cascade** (Ruan et al.). https://arxiv.org/abs/2607.06503. 2026-07-07. read: abstract.
  - Activation probes predict eventual failure from the *first* round. A calibrated cascade gives a certified recall guarantee and saved 54.9-60.2% of tokens at 90% recall (TextCraft and WebShop, models of 3B-7B).
  - **Behaviour-only monitors were consistently weaker, and adding behavioural features to the probes added nothing.**

- **AgentStop: Terminating Local AI Agents Early to Save Energy** (Pham et al.). https://arxiv.org/abs/2605.15206. 2026-05-01 (ACM CAIS '26). read: abstract.
  - Token log-probability signals predict failing runs. Measured: 15-20% less wasted energy at under 5% utility loss.

- Also in this group, all already listed above:
  - Fail-Fast (1.2b): prefix-only, no logits.
  - TACT (1.2b): hidden states, AUC about 0.9.
  - Geometry of Divergence (1.2a): hidden states.
  - STEPGATE (1.3): calibrated gate over observable and logit signals.
  - SAG (1.2a): value of information from action entropy and margin.
  - CogRouter (1.2a): action confidence.
  - Agent-as-a-Router (1.3): task-dimension priors.

### 1.5 Evaluation methodology for routers and for per-step interventions

**Counterfactual per-step replay (fix the prefix, vary one step):**

- **ARES label pipeline** (see 1.2a): freeze a reference prefix and resample step k K=3 times per effort level. The label is the cheapest level that reproduces the reference action.
  - It is cheap because it needs no rollout to the end. But it measures *agreement with one action*, not the final outcome.

- **The Replay Gap** (2608.08239, read: abstract).
  - Live SWE-bench trajectories are forked at controlled depths, the environment is rebuilt, and each fork continues with another model. *Same-model control forks* set the noise floor. About 900 rollouts.
  - Measured:
    - Swaps rewrote 61-94% of actions after the fork.
    - 74-77% of early swaps diverged at the first action, against 6-35% of controls.
    - Divergence shrinks with fork depth.
    - 5 outcome flips occurred, all in swap arms; none of 359 control forks flipped.
    - **A log-stitching replay evaluator mispredicted every success-relevant outcome.**
    - FP8-served controls diverged on more than 90% of forks.
  - Implication: per-step routing must be scored with live forks plus same-configuration control forks, not by stitching logs.

- **Causal Agent Replay: Counterfactual Attribution for LLM-Agent Failures (CAR)** (Shah). https://arxiv.org/abs/2606.08275. 2026-06-06. read: abstract; the list of interventions is snippet only.
  - Treats a run as a structural causal model: intervene on step k with do(·), re-run forward K times under the same stochastic policy, and compare outcome distributions, with confidence intervals.
  - A "point-of-commitment" rule handles the confound that resampling step k also re-rolls every later step. A budget-bounded Monte-Carlo Shapley estimator splits credit across interacting steps.
  - Validated only on synthetic SCMs.
  - Snippet only: the interventions are do_resample (a null control), do_action, do_observation, do_context and do_policy (swap the model from k onward). do_policy maps directly onto "change effort from step k".

- **VoS stepwise outcome table** (see 1.4): steer at every non-terminal step and run each continuation to the end. About 82k continuations. Learning from *measured outcomes* was essential.

- **Thought Anchors: Which LLM Reasoning Steps Matter?** (Bogdan, Macar, Nanda, Conmy). https://arxiv.org/abs/2506.19143. 2025-06-23 (v4 2025-10-27). read: abstract.
  - Black-box counterfactual resampling: for each sentence, resample semantically different replacements, continue to an answer, and measure the shift in the answer distribution.
  - Planning and uncertainty-management sentences carry the most influence.
  - The same method transfers to agent steps.

- **Ask Early, Ask Late, Ask Right** (see 1.2b): a forced-injection design that applies the same intervention at chosen fractions of the run to produce "demand curves" over time.

- **Which Agent Causes Task Failures and When? (Who&When)** (Zhang et al.). https://arxiv.org/abs/2505.00212. 2025-04-30 (ICML 2025). read: abstract.
  - The best automated method finds the responsible agent 53.5% of the time but the decisive step only 14.2%.
  - So LLM-judge attribution of *which step mattered* is unreliable; this motivates counterfactual replay.

**Baselines and cost/quality metrics:**

- RouteLLM (1.3): PGR, APGR and CPT, with a random router at the same cost.
- RouterBench (1.3): AIQ on the convex hull, beating the zero-router mixture of fixed models, and an oracle.
- STEPGATE (1.3): a rate-matched random baseline over 5 seeds, plus a query-level routing baseline and strong-only.
- ARES (1.2a): fixed low, medium and high; uniform random (not matched); prompted frontier-LLM choosers.

- **AI Agents That Matter** (Kapoor et al.). https://arxiv.org/abs/2407.01502. 2024-07-01. read: abstract.
  - Accuracy-only evaluation produces needlessly costly agents. Optimise cost and accuracy jointly, and report Pareto frontiers. HAL (1.1) applies this at scale.

- **SWE-Effi** (Fan et al.). https://arxiv.org/abs/2509.09853. 2025-09-11. read: abstract.
  - Effectiveness metrics that weigh resolve rate against tokens and time. Names "expensive failures": agents burn resources while stuck on tasks they cannot solve.

**Statistics, non-inferiority and equivalence:**

- **Adding Error Bars to Evals** (Miller, Anthropic). https://arxiv.org/abs/2411.00640. 2024-11-01. read: full (recommendations).
  - Report the SEM.
  - Use **clustered SEs** when questions come in related groups (these can be more than 3x the naive SE).
  - Run inference on **question-level paired differences**.
  - Reduce variance by resampling answers K times per question, without lowering temperature.
  - Use **power analysis** and the minimum detectable effect formula to decide whether an eval can test a hypothesis at all.

- **Certifying Compressed Language Models: An Audit and a Statistical Toolkit** (Singh). https://arxiv.org/abs/2608.15046. 2026-08-15. read: abstract.
  - Measured across 1,707 paired cells: **churn (per-item flips) is about 5x the net accuracy delta**.
  - None of 16 audited "equivalent" claims declared a margin.
  - Proposed reporting standard: declare a margin, run a paired test, report churn beside net delta, state the n achieved, and release per-item outputs.

- **CoVeR: Coverage-Based Routing of Verifier Calls in Agentic Retrieval** (Roh, Han). https://arxiv.org/abs/2609.26086. 2026-09-03. read: abstract; NI details snippet only.
  - A gated agent came within a fraction of an EM point of the full-budget agent while dropping 62-68% of verifier calls.
  - Snippet only: non-inferiority was one-sided (lower 95% bound of ΔEM > −2 pp, α = 0.025), equivalence used TOST, margins were fixed before the full run, and Holm correction was applied.

- **When May an Agent Stop? Evidence-Carrying Termination for Tool-Using LLMs (ECT)** (Liu). https://arxiv.org/abs/2608.23623. 2026-08-22. read: abstract.
  - The agent may declare COMPLETE only with a certificate tying each claim to in-scope trace evidence, plus a deterministic replay.
  - Measured in a prespecified closed-loop test of 576 trajectories:
    - Premature unsupported terminations: 0/66 vs 40/66 for the baseline controller.
    - Supported completions: 97/132 vs 92/132 (+3.79 pp, CI [0.00, 9.09]), which **meets a −10-point non-inferiority margin**.
  - The tasks are synthetic.
  - This is a termination gate rather than an effort change, aimed at the failure we saw.

- **Prompt-Induced Waste** (1.2c) is a worked example of the design we need:
  - a stage gate on task difficulty (low success between 30% and 80%);
  - paired McNemar with Holm;
  - **reruns of discordant pairs, which halved the apparent rescues**;
  - preregistered gates that correctly blocked the adaptive arms when there was no headroom.

### 1.6 Vendor documentation (outside the academic scope, but load-bearing for the mechanism)

- **Claude Platform docs, "Effort"**. https://platform.claude.com/docs/en/build-with-claude/effort. Fetched 2026-10-14. read: full.
  - **Per-message effort.** In beta (header `mid-conversation-output-config-2026-07-01`) on Fable 5.1, Mythos 5.1, Opus 5.5, Opus 5, Sonnet 5.5 and Haiku 5.5. It preserves the prompt cache.
  - **Where a per-message change takes effect.** Quote: "A `user` turn that holds only `tool_result` blocks doesn't count as new input, so a change placed after one in a tool-use loop waits for the next `user` turn with new input."
  - **Top-level changes.** A change to the top-level `output_config.effort` "doesn't preserve cached prefixes".
  - **Sonnet 5.5 with `between_tools` thinking.** A differing per-message effort returns a 400 error.
  - Vendor guidance for Sonnet 5.5: for agentic coding, start at `medium` for well-specified tasks and move to `high` for harder or longer ones; use `xhigh` or `max` only where evals show a gain.
  - Vendor claim, about **Haiku 5.5** at `low`: "In long agent prompts, the model is more likely to skip a search, stop early, or skip a check". This has the same shape as our three end-of-run losses, but it is said about a different model.
  - **Open question for us.** Our memory note says the mod writes both the top-level `output_config.effort` and an appended `role: system` pin, and that no cache miss was seen mid-turn. Read against this page, it is not established which level governs the *next request inside a tool loop*.
  - Verify before running a router experiment: measure the thinking and output tokens of the request right after a mid-turn switch, with a positive control.

---

## 2. What this means for our design

### 2.1 Where in a trajectory effort seems to matter

1. **Not uniformly, and the profile depends on the task.**
   - ARES labels: 57% of TAU Retail steps needed only low, but 64% of WebArena steps needed high.
   - CogRouter, TAB, ALAR and RACE all save 35-85% of tokens by thinking on fewer turns.
   - RACE adds a mechanism: reasoning written earlier keeps supporting later actions until the context changes. Routine steps right after a reasoned plan (for example, editing a tracker once the context is gathered) are the cheapest place to drop effort. This is consistent with the operator's intuition.

2. **After a surprise: recovery and re-planning.**
   - In ARES, `go_back` and `branch` steps got the most high effort.
   - The Stuck Monitor (computer-use), STEPGATE's state signals (previous tool error, repeated action) and RACE's "context changed" condition point the same way.
   - For a coding agent, the analogue is the step right after a failing test, a compile error or an unexpected output.

3. **Commit points, as opposed to reads.**
   - Geometry of Divergence finds that the outcome signal depends on the Read/Write/Respond pattern.
   - STEPGATE requires more confidence for state-changing actions.
   - The Milestone Monitor puts verification at semantic checkpoints.
   - These are the steps where a mistake is costly to undo.

4. **The end of a coding run (verification and the decision to stop) is a distinct failure locus, but no study isolates effort there.**
   - Failure studies place many coding failures late: Coherence Collapse (60-69% of failures already had the right functions), SWE-Marathon (premature termination, poor self-verification), Building to the Test (no unprompted validation), SWE-PRM ("failure to terminate"), Danger of Overthinking ("premature disengagement") and Ask Early/Ask Late (asking after the midpoint is worse than never asking).
   - Prompt-Induced Waste is the only effort sweep in our setting, Claude Code with Sonnet/Opus 5. There, the higher-effort rescues that reproduced were **cross-module-completeness failures**, the same shape as our cs-p20 (a field added, other test targets not compiled).
   - Even so, only half of their rescues reproduced, and a simple escalation rule did not move the frontier.
   - Our n=3 "losses at the end" hypothesis is *consistent* with the literature but not established by it.

5. **More effort is not monotone.**
   - ARES Airline: medium 42.0 beat high 38.0.
   - HAL: higher effort did not help in most runs.
   - Inverse Scaling: longer reasoning hurt on some task types.
   - Prompt-Induced Waste: every timeout and empty patch came at high or xhigh, and Opus at xhigh made scope-expanding edits.
   - The router needs a "lower" direction and a turn and time budget, not only escalation.

6. **Effort may be the wrong lever for the stop decision.**
   - Coding Agents Don't Know When to Act found that effort barely moved abstention, while framing did.
   - Rethinking Agent Tests found that prompting test volume did not move outcomes.
   - ECT's termination gate removed premature terminations without hurting completions (synthetic tasks).
   - So an arm with **low effort plus a termination gate** (for example, require a full-suite run or evidence before the final message) is a cheaper competitor that an effort router has to beat.

### 2.2 Router signals with evidence, and which ones we can get

| Signal | Evidence | Available to a Claude Code hook + Jev/JevK5? |
|---|---|---|
| Interaction-history text into a small router fine-tuned on measured per-step labels | ARES (Qwen3-1.7B, SFT then RL) beat fixed high's cost at equal or better accuracy on 3 of 4 benchmarks | Yes (the transcript), but it needs labels from our own replays |
| The same context given to an **untrained, prompted** chooser | ARES: GPT-5 and Gemini 3 Pro choosers scored below fixed medium on 3 of 4 benchmarks | Yes. This is what an un-tuned Jev or JevK5 call is. Treat it as a baseline to beat, not as a router |
| Observable state: previous tool error or failing test, repeated or looping actions, trajectory length, empty observations, retries left | STEPGATE (calibrated gate), Stuck Monitor, Fail-Fast (prefix-only 0.6B monitor, transfers to a closed API model), SWE-Effi "expensive failures" | Yes (tool names, exit codes, edit events) |
| Action type: read vs state-changing vs final message / stop / ask-user | STEPGATE risk tier, Geometry Read/Write, Milestone Monitor, the end-of-run failure studies | Partly. The *next* action is not known when effort must be set; it can be inferred from the previous step, or found with propose-then-gate (STEPGATE) at the cost of an extra request |
| Acting-model logits or hidden states (entropy, top-2 margin, probes) | STEER, STEPGATE, CogRouter, SAG, TACT, Doomed-from-the-Start (behaviour-only monitors weaker), LLMs Encode Failures | Probably not for Claude through Claude Code (not checked here). JevK5's option probabilities are the *router's* confidence, not the agent's |
| Task-level prior (difficulty, repo, task type) | Snell, Agent-as-a-Router (+15.3% relative from task-dimension statistics), LLMs Encode Failures | Yes. But see the next row |
| Caveat | "It's the Problem, Not the Path": a pooled AUROC can be pure difficulty and at chance within a problem. VoS: uncertainty finds failing *runs* but not *the step* where intervening helps | A per-step signal must beat a per-task (difficulty-only) router, evaluated within task |
| Caveat | Beyond Resolution Rates: the length-to-failure association reverses once difficulty is controlled | Do not use raw trajectory length as a "trouble" signal without a difficulty control |

### 2.3 Evaluation designs and baselines the literature uses, mapped to our router

1. **Gate the task set on headroom first.**
   - Prompt-Induced Waste required low-effort success between 30% and 80%. Its adaptive campaign was blocked because low solved everything, exactly like our S5 `effort_idle` result.
   - A router test is uninformative without tasks where fixed-low fails and fixed-high passes. Our R1 V2 had only cs-p20, and cs-p12 partly.

2. **Offline counterfactual table at chosen decision points** (VoS, CAR, ARES, Replay Gap).
   - Snapshot the repo and transcript at step k. Resume at each effort level (do_policy from k onward) K times, run to the end, and grade with hidden tests.
   - **Always add a same-effort control fork (do_resample) to measure the noise floor.**
   - Never score by stitching logs together: the Replay Gap found that this mispredicts every success-relevant outcome.
   - Choose k at the candidate loci from 2.1: the step after a failing test or compile error, and the step that produces the final message or the question to the user.
   - Late forks are cheaper and diverge less (Replay Gap: divergence shrinks with fork depth).
   - The table gives per-step oracle labels and the *value of high effort at step k*. It is also training data if Jev or JevK5 is to be tuned (ARES, VoS).
   - Cheaper proxy: ARES-style action agreement (K=3, no rollout). Use it only for screening; the outcome is what counts.

3. **Online paired comparison, every arm on the same tasks with several seeds.**
   - Fixed low, medium, high and xhigh (expect medium or high to sometimes dominate xhigh).
   - **Per-step random at the router's realised effort mix** (rate-matched, several seeds: STEPGATE, RouteLLM).
   - A **per-task router** (one effort per task: STEPGATE's query-level baseline) to show that per-step adds something.
   - The existing rule (R1).
   - An untrained Jev or JevK5 chooser against a calibrated or trained one.
   - Low effort plus a termination gate (from 2.1.6).
   - The oracle from step 2.

4. **Metrics.**
   - Pass rate on hidden tests, plus USD, tokens, turns and wall time.
   - **PGR** = (router − low) / (xhigh − low), and CPT or APGR if the router has a threshold.
   - The **cost/quality Pareto frontier**: the router must lie above the convex hull of fixed-effort mixtures (RouterBench's zero router), not just above one fixed arm.

5. **Statistics.**
   - Paired by task (exact McNemar or a paired bootstrap), with Holm across arms.
   - A **margin declared in advance with a one-sided lower bound** (non-inferiority, as in our R1 and in ECT and CoVeR).
   - Report **churn** (rescues and regressions), not only the net delta (Certifying: churn is about 5x net).
   - **Rerun discordant pairs** before believing them (Prompt-Induced Waste: 3 of 6 rescues reproduced).
   - Clustered SEs by task for repeated runs, and a power or minimum-detectable-effect check before spending (Miller).

6. **Evaluate router signals within task** ("It's the Problem"). Show that the router's choice predicts the counterfactual gain at a step better than a task-only predictor does.

### 2.4 Risks the literature flags for this design

- Learned mode selectors collapse onto one mode (Thinkless, CogRouter): watch the router's effort distribution.
- The router itself costs something (ARES: the prompted GPT-5 chooser's own cost was high). A local JevK5 at 4B is cheap, but count its latency per request.
- Mid-turn switching: a model swap rewrites most later actions (Replay Gap). An effort switch within one model is presumably milder, but that is not measured. Also confirm that the switch actually takes effect inside a tool loop (1.6).

---

## 3. Not found (queries tried) and leads not opened

**Negative results, each with the searches behind it:**

1. **No study varies reasoning effort *only* at the verification or stop step of a coding agent with a fixed prefix and grades the result with hidden tests.**
   - Queries:
     - "arXiv vary reasoning effort only at final verification step coding agent counterfactual fixed prefix SWE-bench effort switch mid-trajectory"
     - "arXiv per-step reasoning effort allocation LLM agent trajectory when to think tool use"
     - "arXiv SWE-bench reasoning effort low medium high resolve rate cost ablation coding agent thinking budget study"
     - the researcher MCP query "Where in a coding agent's trajectory does extra reasoning effort change task outcome …"
   - Closest: ARES (web and tool tasks, action-agreement labels), the Replay Gap (model swap, not effort), Prompt-Induced Waste (effort fixed for the whole run).

2. **No academic study of mid-turn effort switching in Claude Code or with Anthropic's per-message effort.** Same queries as item 1. The only material is the vendor documentation in 1.6.

3. **No academic evaluation of TypeSafe Jev or JevK5 as a router.**
   - Query: "TypeSafe Jev typed decision model JevK5 Qwen3.5-4B router". It returned only GitHub, OpenRouter and MarkTechPost pages, none opened.
   - Snippet only: TypeSafe advertises a "Jev Router" that picks model and reasoning effort per request; there is also a community `jev-router` for Claude Code that works per human turn.

4. **No probe or predictor of the *marginal gain* from more effort at an agent step that uses only observable state.**
   - Queries:
     - "arXiv probe hidden states predict whether extended reasoning will improve answer before generation difficulty estimation thinking budget"
     - "arXiv 2026 value of computation metareasoning LLM agent decide when to deliberate per step expected improvement"
   - Closest: Damani (per query), VoS (value of steering, from measured outcomes), ARES (minimum sufficient effort from action agreement).

5. **No non-inferiority design applied specifically to an effort or model router inside agents.**
   - Query: "non-inferiority test LLM evaluation cheaper model margin equivalence paired arXiv".
   - Found only adjacent uses: ECT (termination), CoVeR (verifier routing), Certifying (compressed models).

6. **No paper measuring whether effort changes an agent's "stop and ask the user" behaviour.**
   - Query: "arXiv coding agent stops to ask user clarification instead of completing task autonomy premature question reasoning effort".
   - Related but not effort-specific: Ask Early/Ask Late, Agentic Abstention, Coding Agents Don't Know When to Act.

**Other queries run (all productive, listed for completeness):**
- "arXiv overthinking agentic tasks reasoning-action dilemma SWE-bench"
- "arXiv step-level model routing within agent trajectory small large model per step"
- "Holistic Agent Leaderboard reasoning effort accuracy arXiv 2025"
- "arXiv SWE agents testing behavior empirical study agents run tests before submitting premature termination trajectories"
- "arXiv "When Agents go Astray" course-correcting SWE agents process reward models"
- "arXiv AgentStop LLM agent learning when to stop early termination"
- "arXiv counterfactual replay agent trajectory intervene single step resample continuation measure effect on task success"
- "AgentTTS test-time compute-optimal scaling multi-stage tasks arXiv budget allocation per stage"
- "arXiv router selects reasoning effort level or thinking mode per query (low medium high) learned router reasoning models cost"
- "arXiv agent interleaved thinking ablation per-turn thinking vs non-thinking multi-turn tool use which turns benefit from thinking"
- "SWE-Effi re-evaluating software AI agent effectiveness under resource constraints arXiv"

**Leads seen in search results but NOT opened (nothing above rests on them):**
- MiniMax-M2 report 2605.26494 (interleaved-thinking ablation; snippet: keeping earlier thinking helped SWE-bench Verified 69.4 vs 67.2).
- Nudgeability 2609.34572 (paired fixed-prefix design).
- Before They Can Solve 2610.10478 (fork at the decisive step).
- SWE-Replay 2601.22129.
- SWE-Milestone 2603.13428: abstract opened, but the snippet claim that verification effort peaks at 70-80% of the run is not in the abstract and was not verified.
- Thinking Before Thinking 2609.38147.
- ATLAS 2606.01667.
- The LLM Already Knows 2509.12886.
- Predictive Scheduling 2602.01237.
- DiffAdapt 2510.19669.
- SWE-Next 2603.20691 (submit gating: at least one test command before submit).
- An Empirical Study of Harness Design for Coding Agents 2609.20804.
- CausalFlow 2605.25338.
- Ask or Assume? 2603.26233.
- Understanding Code Agent Behaviour 2511.00197.
- Agentic Coding Needs Proactivity 2605.06717.
