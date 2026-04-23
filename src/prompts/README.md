# Prompt surfaces — editing guide

Read this when touching `server_instructions.md`, `onboarding_prompt.md`, or `builders.rs`. Operational rules about *which* surfaces exist and *when* to bump `ONBOARDING_VERSION` live in the top-level `CLAUDE.md` § Prompt Surface Consistency — this file is the **style guide** for the writing itself.

## Surfaces

- `src/prompts/server_instructions.md` — injected **once at MCP session start**, not per-request. Token cost is session-scoped, not per-call — invest in clarity over brevity.
- `src/prompts/onboarding_prompt.md` — one-time onboarding, read only when a project is activated for the first time.
- `build_system_prompt_draft()` in `src/prompts/builders.rs` — generated per-project and embedded into the project's system prompt via onboarding.

## Rules for editing `server_instructions.md`

1. **Cap hard rules at 5–8.** Beyond 8 behavioral constraints, compliance on all drops. Consolidate, don't accumulate.
2. **No triple-layer repetition.** A rule in Iron Laws should NOT be restated in Anti-Patterns AND Rules. Max 2 appearances: once as a law, optionally once as a closing reminder (for the 1–2 most-violated rules only).
3. **Tables > prose** for decision-matrix content. Claude scans tables faster.
4. **End of prompt = highest compliance.** Put the most-violated rule(s) in the closing `## Rules` section — that's closest to generation.
5. **Don't document every param.** Pagination (`offset`, `limit`, `detail_level`) and aliases (`file_path`, `limit`) are discoverable from the tool schema. Only document params that change behavior in non-obvious ways.
6. **Prompt caching matters.** Keep section order stable between releases so the static prefix benefits from automatic caching. Don't reorganize for cosmetic reasons.
7. **You are the consumer.** When writing or reviewing prompt changes, think as the agent who will read this mid-task. Ask: "Would this have helped me find the right tool chain naturally?" Test by simulating a realistic task and checking whether the prompt guided you to the right flow. Usage data (`usage.db`) is the ground truth — if a tool has near-zero calls despite being useful, the prompt isn't surfacing it.

## Research

Evidence behind these rules:

- `docs/research/2026-03-21-claude-prompt-engineering.md`
- `docs/research/2026-03-21-superpowers-prompt-patterns.md`
