---
specialist: architecture-snow-lion
scope: project
slug: agentic-surface-as-moat
created: 2026-05-07
updated: 2026-05-07
tags: [agentic, prompts, moat, llm-facing, weight-of-change]
---

**Lesson:** This project's strongest architectural dimension is its agentic-knowledge surface — the Iron Laws and anti-pattern table in `src/prompts/server_instructions.md`, the decision trees, the `docs/TODO-tool-misbehaviors.md` living log, the three-query-sandwich test pattern, the `RecoverableError` vs `anyhow::bail!` propagation split. Backend code quality is solid Rust idiom; surface design is what differentiates this project from the median MCP server. The moat lives in the LLM-facing layer.

**Why:** Most MCP servers expose tools and pray. This project encodes failure modes empirically observed across LLM-tool interactions and forces them into structure: "violating the letter IS violating the spirit," anti-patterns named with their concrete failure modes, the misbehavior log mandated as a workflow step. That's not documentation — it's a feedback loop that compounds over sessions.

**How to apply:** When weighing any architectural change, weight changes that affect the LLM-tool interaction surface heavier than equivalent backend changes. Tool renames, error-message reshaping, hint-text edits, anti-pattern table edits — these are higher-stakes than pure internal refactors. Before opining on any refactor, ask: does this preserve, extend, or weaken the agentic moat? A change that simplifies internals at the cost of surface quality is a net regression even if the diff looks clean. The agentic surface is what brings the project to life, and what it loses is hard to win back.
