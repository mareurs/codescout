# Prompt surfaces — editing guide

Read this when touching `source.md` (the single source for the `server_instructions` and `onboarding_prompt` surfaces) or `builders.rs`. Operational rules about *which* surfaces exist and *when* to bump `ONBOARDING_VERSION` live in the top-level `CLAUDE.md` § Prompt Surface Consistency — this file is the **style guide** for the writing itself.

## Surfaces

- `src/prompts/source.md` — the **single editable document** for the next two surfaces. `build.rs` slices it into `OUT_DIR` at compile time; `src/prompts/source.rs::extract_surface` is the matching runtime parser. Edit here.
  - `server_instructions` surface — injected **once at MCP session start**, not per-request. Token cost is session-scoped, not per-call — invest in clarity over brevity.
  - `onboarding_prompt` surface — one-time onboarding, read only when a project is activated for the first time.
- `build_system_prompt_draft()` in `src/prompts/builders.rs` — generated per-project and embedded into the project's system prompt via onboarding.
## Rules for editing the `server_instructions` surface

1. **Cap hard rules at 5–8.** Beyond 8 behavioral constraints, compliance on all drops. Consolidate, don't accumulate.
2. **No triple-layer repetition.** A rule in Iron Laws should NOT be restated in Anti-Patterns AND Rules. Max 2 appearances: once as a law, optionally once as a closing reminder (for the 1–2 most-violated rules only).
3. **Tables > prose** for decision-matrix content. Claude scans tables faster.
4. **End of prompt = highest compliance.** Put the most-violated rule(s) in the closing `## Rules` section — that's closest to generation.
5. **Don't document every param.** Pagination (`offset`, `limit`, `detail_level`) and aliases (`file_path`, `limit`) are discoverable from the tool schema. Only document params that change behavior in non-obvious ways.
6. **Prompt caching matters.** Keep section order stable between releases so the static prefix benefits from automatic caching. Don't reorganize for cosmetic reasons.
7. **You are the consumer.** When writing or reviewing prompt changes, think as the agent who will read this mid-task. Ask: "Would this have helped me find the right tool chain naturally?" Test by simulating a realistic task and checking whether the prompt guided you to the right flow. Usage data (`usage.db`) is the ground truth — if a tool has near-zero calls despite being useful, the prompt isn't surfacing it.
8. **2200-byte hard cap on the static slice.** The `server_instructions` slice is delivered as the MCP `initialize.instructions` field, which Claude Code silently truncates at ~2000 bytes (see `docs/architecture/mcp-channel-caps.md`). The cap is enforced by `prompts::redesign_invariants::source_md_under_cap` (`src/prompts/mod.rs:1037-1046`) with `MAX_INSTRUCTIONS_CHARS = 2200` — 200 bytes of headroom for the dynamic `## Project Status` block runtime-appends. When the test fails, do NOT raise the cap — author a `get_guide(topic)` entry and reference it from the slice. (Don't put a literal "EDITOR NOTE" HTML comment containing the surface/end marker strings into `source.md` itself; the extractor at `src/prompts/source.rs::extract_surface` does a substring `find` and will match the comment first, breaking the slice — F-5 in `docs/trackers/prompt-guide-refactor-session-log.md`.)

## Research

Evidence behind these rules:

- `docs/research/2026-03-21-claude-prompt-engineering.md`
- `docs/research/2026-03-21-superpowers-prompt-patterns.md`
