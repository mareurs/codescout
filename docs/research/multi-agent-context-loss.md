---
title: Context Loss and Compound Error in Multi-Agent LLM Systems
date: 2026-03-15
topic: agent-architecture
summary: Empirical and theoretical analysis of compound error in delegation trees vs single-session skill-based architectures.
status: complete
---

# Context Loss and Compound Error in Multi-Agent LLM Systems

> Research compiled March 2026. Sources include peer-reviewed papers (arXiv),
> industry engineering blogs, and empirical benchmarks.

---

## Executive Summary

The intuition is correct: when multiple AI agents each lose ~20% of context or accuracy,
the compound error rate grows exponentially — not linearly. A 5-agent chain where each
agent is 80% accurate yields ~33% end-to-end accuracy (0.80^5 = 0.328), not 80%.
Peer-reviewed research confirms failure rates of 41–87% in production multi-agent systems,
and a 50-percentage-point accuracy gap between single agents with full context vs.
multi-agent systems with distributed information.

This has direct implications for choosing between multi-agent orchestration (deep delegation
tree patterns) and single-session skill-based workflows (codescout's approach).

---

## 1. The Mathematics of Compound Error

### The 0.95^N Problem

If each agent in a sequential chain has 95% accuracy, the system accuracy after N steps is:

| Agents in chain | System accuracy |
|-----------------|-----------------|
| 1               | 95.0%           |
| 3               | 85.7%           |
| 5               | 77.4%           |
| 10              | 59.9%           |
| 15              | 46.3%           |

**At 80% per-agent accuracy (the user's ~20% loss assumption):**

| Agents in chain | System accuracy |
|-----------------|-----------------|
| 1               | 80.0%           |
| 3               | 51.2%           |
| 5               | 32.8%           |
| 7               | 21.0%           |
| 10              | 10.7%           |

The user's intuition of ">80% error" with multiple agents losing ~20% each is
mathematically validated: five agents at 80% accuracy each produce ~67% error rate.

**Source:** [Why Multi-Agent AI Fails: The 0.95^10 Problem](https://www.artiquare.com/why-multi-agent-ai-fails/)

### Why It's Worse Than Simple Multiplication

Errors don't just pass through — they *amplify*. An error at step 3 corrupts the input
to step 4, which amplifies the error at step 5. By step 8, you're not debugging a model —
you're debugging chaos. This is because each agent makes decisions based on the
(potentially flawed) output of the previous agent, creating a "composition crisis."

**Source:** [Why Your Multi-Agent System is Failing: Escaping the 17x Error Trap](https://towardsdatascience.com/why-your-multi-agent-system-is-failing-escaping-the-17x-error-trap-of-the-bag-of-agents/)

---

## 2. The Telephone Game Effect

### The Term

Christopher Yee coined "The Agentic Telephone Game" to describe how AI output layered
on AI output without human checkpoints introduces compounding drift — small accuracy gaps
that accumulate while the output remains fluent and confident.

### The Math

Assuming each AI interaction preserves ~92% accuracy, four rounds of AI-on-AI processing
degrades accuracy to ~72%, even though the output *looks* like it's at 100%. The danger
is that unlike manual work where errors are visible, AI output "decouples effort from
quality" — the text reads beautifully while the facts drift.

### Real-World Example

A strategy document passed through 5 LLM iterations (draft → revise → review → finalize,
each via LLM). Result: "a document that read beautifully, flowed logically" but was built
on unverified foundational assumptions. The entire document was scrapped.

### Speed Makes It Worse

- **Human telephone game:** Takes days, allowing course-correction
- **Agent telephone game:** Runs in minutes with zero opportunity for organic correction

**Source:** [The Agentic Telephone Game: Cautionary Tale](https://www.christopheryee.org/blog/agentic-telephone-game-cautionary-tale/)

---

## 3. Empirical Evidence: Multi-Agent Failure Rates

### Cemri et al. (2025) — "Why Do Multi-Agent LLM Systems Fail?"

The most comprehensive study to date analyzed 5 popular multi-agent frameworks and found:

- **Failure rates: 41% to 86.7%** across 7 state-of-the-art open-source systems
- **14 distinct failure modes** organized into 3 categories
- Tactical improvements (prompt refinement, topology redesign) yielded only **14%
  performance gains** — insufficient for production use

**Failure mode taxonomy (MASFT):**

| Category | Share | Key failures |
|----------|-------|-------------|
| Specification & system design | 41.8% | Role disobedience, conversation history loss, step repetition |
| Inter-agent misalignment | 36.9% | Conversation resets, task derailment, ignoring peer input |
| Task verification gaps | 21.3% | Premature termination, incomplete verification |

Critical finding: **"Conversation history loss"** — agents experience "unexpected context
truncation, disregarding recent interaction history and reverting to antecedent
conversational state." This is the telephone game at the architectural level.

**Source:** [Why Do Multi-Agent LLM Systems Fail?](https://arxiv.org/html/2503.13657v1)
(arXiv 2503.13657)

### Production Failure Rates (Augment Code analysis)

- **41–86.7% of multi-agent LLM systems fail in production**, with most breakdowns
  occurring within hours of deployment
- **79% of problems** originate from specification and coordination issues, not
  technical implementation
- PwC case study: structured architecture improved code generation accuracy from
  **10% to 70%** (7x), but only with independent validation at each step

**Source:** [Why Multi-Agent LLM Systems Fail and How to Fix Them](https://www.augmentcode.com/guides/why-multi-agent-llm-systems-fail-and-how-to-fix-them)

---

## 4. The Distributed Information Problem

### Collective Reasoning Failures (arXiv 2505.11556)

This study directly measures what happens when information is split across agents:

| Condition | Accuracy |
|-----------|----------|
| Single agent, complete information | **80.7%** |
| Multi-agent, distributed information | **30.1%** |
| Multi-agent, all information revealed upfront | **96.7%** |

The 50.6-percentage-point gap exists *despite* agents being individually capable.
The problem is not reasoning — it's **information surfacing**. Agents cannot recognize
what others know but haven't shared.

Worse: **performance degrades as you add more agents** (improvement drops from +0.348
at 3 agents to +0.006 at 7 agents). More agents = more information boundaries = more loss.

**Source:** [Systematic Failures in Collective Reasoning under Distributed Information](https://arxiv.org/html/2505.11556v3)

---

## 5. Context Rot: The Underlying Mechanism

### Chroma Research — "Context Rot"

Research from Chroma demonstrates that LLMs do not process context uniformly. As input
length increases, reliability declines — even on simple tasks:

- **Lower semantic similarity** between question and answer causes steeper performance decline
- **Distractors amplify** degradation at scale (non-uniformly — some are far worse)
- **Structured text performs worse** than shuffled/incoherent content (attention mechanisms
  misallocate focus based on structural patterns)
- Claude models abstain when uncertain; GPT models generate confident but incorrect answers

**Implication for multi-agent systems:** Each handoff reconstructs context in a new window.
The receiving agent gets a summary/instruction, not the original reasoning. This is
context rot applied at every boundary.

**Source:** [Context Rot: How Increasing Input Tokens Impacts LLM Performance](https://research.trychroma.com/context-rot)

### JetBrains Research — Context Management for Agents

Empirical study on 500 SWE-bench instances comparing context management strategies:

- Both context management approaches cut expenses by **>50%** vs unmanaged contexts
- **Observation masking matched or exceeded LLM summarization** in 4 of 5 configurations
- Summarization caused agents to run **13–15% longer** (obscured stopping signals)
- Summary generation consumed **>7% of total costs** while performing no better

**Key finding:** Sophisticated summarization sometimes backfires by obscuring signals,
paying more for equivalent or worse results. Simpler approaches (masking) often win.

**Source:** [Cutting Through the Noise: Smarter Context Management](https://blog.jetbrains.com/research/2025/12/efficient-context-management/)

---

## 6. The Counter-Argument: When Multi-Agent Works

### Anthropic's Own Multi-Agent System

Anthropic built a multi-agent research system and published findings:

- Multi-agent with Claude Opus 4 lead agent **outperformed single-agent by 90.2%**
  on internal evaluations
- But: agents use **~4x more tokens than chat**, and multi-agent uses **~15x more tokens**
- Architecture mitigates telephone game by having subagents "store work in external
  systems, then pass lightweight references back to the coordinator"
- Explicit memory mechanisms save research plans to external storage when approaching
  200K token limits

**Critical design choice:** Rather than passing full context through the chain (telephone
game), they pass *references* to externally stored artifacts. The coordinator never
reconstructs another agent's full reasoning — it reads the output artifact directly.

**Source:** [Anthropic Engineering: Multi-Agent Research System](https://www.anthropic.com/engineering/multi-agent-research-system)

### Hybrid Approaches (arXiv 2505.18286)

Research shows the benefits of multi-agent systems over single-agent **diminish as LLM
capabilities improve**. A hybrid cascading design:

- Routes easy requests to single-agent, hard requests to multi-agent
- Improves accuracy by **1.1–12%** while reducing costs by **up to 20%**
- Key insight: "the benefits of MAS over SAS diminish as LLM capabilities improve"

**Source:** [Single-agent or Multi-agent Systems? Why Not Both?](https://arxiv.org/abs/2505.18286)

---

## 7. Architectural Implications

### The Deep Delegation Tree Pattern

Some production AI coding assistants implement deep delegation trees where a top-level
orchestrator spawns specialized sub-agents for each phase of a workflow. A representative
example:

```
orchestrator
├── scanner        (sub-agent)
├── researcher     (sub-agent)
├── analyzer       (sub-agent)
├── strategist     (sub-agent)
├── intent-parser  (sub-agent)
├── planner        (sub-agent)
└── coder          (sub-agent)
```

This is a 7-agent chain. At 90% per-agent accuracy: 0.90^7 = **47.8% system accuracy**.
At 85%: 0.85^7 = **32.1%**. Each sub-agent gets a compressed summary of the orchestrator's
intent — classic telephone game topology.

### The Single-Session Skill-Based Pattern

An alternative architecture — exemplified by codescout's approach — uses **skills, not
agents** for the core workflow:

```
brainstorming → planning → implementation → finishing
```

Skills execute in the **same context window** as the main session. No inter-agent
handoff = no telephone game. The only delegation is to focused sub-agents for
isolated tasks (e.g. code review), where context loss is bounded because the sub-agent
receives the specific artifact (diff/file) rather than a summary of prior reasoning.

### Why Skills Beat Agent Trees for Context Preservation

| Factor | Multi-agent tree | Single-session skills |
|--------|-----------------|----------------------|
| Context continuity | Broken at every handoff | Preserved across skills |
| Error compounding | Multiplicative (0.9^N) | Additive (single session) |
| Information loss | Summary/instruction at each boundary | Full conversation history |
| Telephone game risk | High (N boundaries) | Minimal (0–1 boundaries) |
| Token cost | ~15x chat baseline | ~4x chat baseline |
| Debugging | Distributed across agents | Single conversation trace |
| When better | Truly independent parallel tasks | Sequential reasoning chains |

### When to Use Sub-Agents

Sub-agents are appropriate when:
1. The task is **truly independent** (code review of a specific file)
2. The sub-agent receives the **actual artifact**, not a summary of reasoning
3. The result is **verifiable** (tests pass/fail, lint clean/dirty)
4. Context isolation is a **feature** (preventing contamination of main session)

Following Anthropic's pattern: pass *references to artifacts*, not reconstructed context.

---

## 8. Recommendations

1. **Default to single-session skills** for sequential workflows. The compound error
   math is unforgiving — every handoff is a potential 5–20% accuracy loss.

2. **Use sub-agents only for isolated, verifiable tasks** where the input is a concrete
   artifact (file, diff, test suite) and the output is boolean or structured.

3. **Never pass summaries of reasoning between agents.** Pass references to stored
   artifacts (files, plans, test results). This is what Anthropic does internally.

4. **Add human-in-the-loop checkpoints** at workflow boundaries (brainstorming → plan,
   plan → implementation). Each checkpoint resets the accuracy baseline.

5. **Measure end-to-end accuracy**, not per-agent accuracy. A system of five 90%-accurate
   agents is a 59%-accurate system, not a 90%-accurate one.

---

## Sources

### Peer-Reviewed / arXiv

- Cemri, Pan, Yang et al. — [Why Do Multi-Agent LLM Systems Fail?](https://arxiv.org/html/2503.13657v1) (arXiv 2503.13657, 2025)
- [Systematic Failures in Collective Reasoning under Distributed Information in Multi-Agent LLMs](https://arxiv.org/html/2505.11556v3) (arXiv 2505.11556, 2025)
- [Single-agent or Multi-agent Systems? Why Not Both?](https://arxiv.org/abs/2505.18286) (arXiv 2505.18286, 2025)
- [Memory Management and Contextual Consistency for Long-Running Low-Code Agents](https://arxiv.org/pdf/2509.25250) (arXiv 2509.25250, 2025)

### Industry Research

- Anthropic Engineering — [Building a Multi-Agent Research System](https://www.anthropic.com/engineering/multi-agent-research-system)
- Chroma Research — [Context Rot: How Increasing Input Tokens Impacts LLM Performance](https://research.trychroma.com/context-rot)
- JetBrains Research — [Cutting Through the Noise: Smarter Context Management for LLM-Powered Agents](https://blog.jetbrains.com/research/2025/12/efficient-context-management/)

### Practitioner Analysis

- Christopher Yee — [The Agentic Telephone Game: Cautionary Tale](https://www.christopheryee.org/blog/agentic-telephone-game-cautionary-tale/)
- Artiquare — [Why Multi-Agent AI Fails: The 0.95^10 Problem](https://www.artiquare.com/why-multi-agent-ai-fails/)
- Towards Data Science — [Why Your Multi-Agent System is Failing: Escaping the 17x Error Trap](https://towardsdatascience.com/why-your-multi-agent-system-is-failing-escaping-the-17x-error-trap-of-the-bag-of-agents/)
- Augment Code — [Why Multi-Agent LLM Systems Fail and How to Fix Them](https://www.augmentcode.com/guides/why-multi-agent-llm-systems-fail-and-how-to-fix-them)
- Galileo AI — [Why Do Multi-Agent LLM Systems Fail](https://galileo.ai/blog/multi-agent-llm-systems-fail)
