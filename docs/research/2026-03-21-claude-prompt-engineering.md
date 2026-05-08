---
title: Claude System Prompt Engineering Research
date: 2026-03-21
topic: prompt-engineering
summary: Patterns and anti-patterns observed across Claude system prompts in production agentic tools.
status: complete
---

# Claude System Prompt Engineering Research

**Date:** 2026-03-21
**Context:** Audit of `src/prompts/server_instructions.md` — optimizing for token efficiency and LLM compliance.

## Key Findings

### 1. Redundancy Hurts Claude More Than It Helps

Research on prompt compression (LLMLingua, CompactPrompt) shows natural language contains
massive redundancy that can be stripped with minimal performance loss. For Claude specifically:

- Repeating the same instruction in different words **dilutes signal-to-noise** and wastes
  the attention budget.
- **Exception:** Restating a critical constraint once at the top (as a rule) and once at the
  bottom (as a reminder) can help due to primacy/recency effects. But limit this to 1–2
  truly critical rules, not a general pattern.
- Structured formats (tables, JSON, XML) are inherently less redundant than prose and are
  preferred by Claude.

### 2. Hard Rules Should Be Capped at 5–8

Beyond 8 behavioral constraints, compliance on all of them drops. This is a direct tension
with having 5 Iron Laws + 13 Rules (= 18 directives). The fix: consolidate, don't accumulate.

### 3. Prompt Position Matters (Primacy/Recency)

Anthropic's long-context research confirms:

- **End of prompt = highest adherence** during generation (closest to where generation begins).
- **Top of prompt = also well-attended** (primacy effect).
- **Middle can suffer** from reduced recall ("lost in the middle"), but this is negligible
  below ~20K tokens. At ~4000 tokens our Tool Reference section is safe in the middle.

**Layout recommendation:**
1. Top: identity + absolute constraints (Iron Laws)
2. Middle: structured reference material (tool tables, decision trees)
3. Bottom: the 1–2 most important behavioral rules restated as closing reminder

### 4. Tables > Prose for Decision-Making

Tables outperform prose for decision-matrix content. Claude scans tables faster and uses
them as lookup structures. Red-flag tables (anti-pattern → correct pattern) are particularly
effective.

### 5. XML Tags for Section Boundaries

Anthropic's top recommendation: use XML tags to separate content types. Each section should
have a single purpose. This helps Claude attend to the right section for the right task.

### 6. Few-Shot Examples > Explanations

For behavioral steering in ambiguous cases, 1–2 concrete examples are worth more than
paragraphs of explanation. Add examples only for the trickiest cases, not for obvious ones.

### 7. Prompt Caching Consideration

Keep the static prefix stable so it benefits from automatic caching. Do not randomize or
rotate the order of system prompt sections between calls.

## Recommended Prompt Skeleton

```
<constraints>
1. NEVER do X.
2. ALWAYS do Y before Z.
3. When [condition], use [tool] — not [other tool].
[5–8 items max, each one sentence]
</constraints>

<tool-routing>
| Situation | Tool | Why |
[Decision matrix — 10–15 rows]
</tool-routing>

<tool-reference>
[Structured tool descriptions, params, usage notes]
</tool-reference>

<patterns>
[2–3 concrete examples of correct behavior for trickiest cases]
</patterns>

<reminder>
[Restate 1–2 most-violated constraints — this is highest-compliance real estate]
</reminder>
```

## Sources

- Anthropic docs: docs.anthropic.com (system prompt design, long-context best practices)
- Anthropic engineering blog: context engineering post
- LLMLingua / CompactPrompt research on prompt compression
- Superpowers plugin patterns (see companion research file)
