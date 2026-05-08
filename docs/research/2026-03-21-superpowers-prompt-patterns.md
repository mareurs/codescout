---
title: Superpowers Plugin — Prompt Architecture Patterns
date: 2026-03-21
topic: prompt-engineering
summary: Architectural patterns used by the Superpowers plugin's skills and slash commands.
status: complete
---

# Superpowers Plugin — Prompt Architecture Patterns

**Date:** 2026-03-21
**Source:** `/home/marius/work/claude/playground/superpowers/`
**Context:** Pattern analysis for improving `src/prompts/server_instructions.md`.

## Patterns Worth Adopting

### 1. Iron Laws as Memorable Single Sentences

ALL-CAPS single-sentence rules beat paragraphs. One memorable rule > 5 paragraphs of context.
Superpowers uses this consistently across skills (TDD, verification, debugging).

### 2. Red-Flag Tables for Rationalization Prevention

```markdown
| Thought | Reality |
|---------|---------|
| "This is just a simple question" | Questions are tasks. Check for skills. |
| "I need more context first" | Skill check comes BEFORE clarifying questions. |
```

Anticipates 10+ common excuses as a decision matrix. LLM uses table lookup instead of
debating internally. **We already do this in the Anti-Patterns table — keep it.**

### 3. Description = Triggering, Not Summary

Skill descriptions ONLY describe triggering conditions ("Use when..."), NEVER summarize
the process. Testing showed that when descriptions summarize workflow, Claude reads the
description and *skips the full body*. 

**Implication for us:** Tool descriptions in server_instructions should say *when to use*
the tool, not repeat the tool's full parameter list.

### 4. Token Budgets Are Real

Superpowers enforces hard limits:
- Getting-started workflows: <150 words each
- Frequently-loaded skills: <200 words total
- Other skills: <500 words

**Implication:** Our server_instructions (~3300 words) is injected on EVERY MCP request.
Every word costs tokens on every call. Trim aggressively.

### 5. Cross-References Prevent Duplication

Link to other skills/sections instead of repeating workflow details. "See section X" beats
restating section X.

### 6. Progressive Disclosure in Content Structure

Skills follow consistent layered structure:
1. **Headline level:** Name + description → Is this relevant?
2. **Section level:** When to Use + Iron Law → Am I in the right situation?
3. **Detail level:** The Process + Common Mistakes → How do I do this?

**Implication:** Our Tool Reference already does this (compact → `detail_level: "full"`).
Apply the same principle to the instructions themselves: scannable headings, details only
where needed.

### 7. DOT Flowcharts for Decision Trees

Graphviz DOT diagrams are token-efficient alternatives to narrative decision descriptions.
LLMs understand them well. Consider for the "How to Choose the Right Tool" section.

## Patterns We Already Have (Keep)

- Tables for decision-making (Anti-Patterns, By-knowledge-level)
- Iron Laws as non-negotiable constraints
- Progressive disclosure (exploring → focused mode)
- Structured tool reference with params

## Patterns to Avoid

- Over-layered repetition (Iron Laws → Anti-Patterns → Rules all saying the same thing)
- Prose descriptions where tables suffice
- Documenting every parameter (pagination, aliases) — let the tool schema speak
