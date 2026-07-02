# Prompt surfaces — editing guide

Read this when touching `source.md` (the single source for the `server_instructions` and `onboarding_prompt` surfaces) or `builders.rs`. This file is the **canonical home** for prompt-surface rules: which surfaces exist (§ Surfaces), when to bump `ONBOARDING_VERSION` (§ Versioning), the writing style guide (§ Rules), and the shared-branch slice hazard (§ Verify the slice). `CLAUDE.md` carries only a one-line pointer here; memory `conventions` § Prompt Surface Consistency has the short version.

**Any change to tool behavior or signatures requires a prompt-surface review** — adding/renaming tools, changing parameter semantics, new error/fallback modes, or changed response shapes. Ask: "Does the LLM need to know this to use the tool correctly?" If yes, update all surfaces in the same commit. The build-time test `server::tests::prompt_surfaces_reference_only_real_tools` catches stale tool-name mentions across the three surfaces; `prompts::tests::claude_md_contains_no_deprecated_tool_names` guards `CLAUDE.md`. ("Distance from change": files closer to a rename get updated, distant ones accumulate stale refs — the tests are the backstop.)

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

## Versioning — when to bump ONBOARDING_VERSION

Bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs` when changing a surface that produces the **stored per-project system prompt** — the `onboarding_prompt` slice of `source.md`, or `build_system_prompt_draft()` in `builders.rs`. The bump triggers automatic system-prompt regeneration for all projects onboarded with the previous version.

**Do NOT bump for `server_instructions` changes** — that surface is injected fresh at every MCP session start (each `/mcp` connect re-reads the sliced text). No cached copy; changes are live on next connect.

| Surface | How delivered | Bump needed? |
|---|---|---|
| `server_instructions` slice of `source.md` | Loaded fresh at every MCP session start | **No** — live on next connect |
| `onboarding_prompt` slice of `source.md` | Drives stored system-prompt generation | **Yes** — cached per project |
| `build_system_prompt_draft()` in `builders.rs` | Same — generates stored system prompt | **Yes** — cached per project |

**Bump when:** tool names change (rename/consolidate); parameter semantics change in the `onboarding_prompt` surface or `builders.rs`; onboarding templates change in ways affecting the generated system prompt.

**Do NOT bump for:** any `server_instructions` change (however significant); bug fixes that don't change tool behavior; internal refactors; memory-template changes (memories are re-read during refresh anyway).

## Verify the slice before committing (shared-branch hazard)

The `server_instructions` slice is under a hard **2200-byte cap** (rule 8, enforced by `prompts::redesign_invariants::source_md_under_cap`). Two ways it bites:

- **Run `cargo test --lib prompt` before any prompt-surface edit is ready to commit.** If `source_md_under_cap` fails, do NOT raise the cap or bless the snapshot to match — move content to a `get_guide(topic)` and leave a pointer in the slice.
- **On a shared branch, re-measure the slice on *current* HEAD.** A concurrent commit can grow the slice under you. `git log --oneline -1` first, then re-check the byte count before trusting any earlier measurement or running `UPDATE_PROMPT_SNAPSHOTS=1` — otherwise you bless the over-cap state into the fixture and ship a truncated slice. Datapoints: F-4 (2026-05-28) and F-8/W-5 (2026-05-31) in `docs/trackers/prompt-guide-refactor-session-log.md`.

## Research

Evidence behind these rules:

- `docs/research/2026-03-21-claude-prompt-engineering.md`
- `docs/research/2026-03-21-superpowers-prompt-patterns.md`
