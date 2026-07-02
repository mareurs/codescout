# codescout — Conventions

## Pre-Commit Requirements

1. `cargo fmt`
2. `cargo clippy -- -D warnings`
3. `cargo test`
All three must pass. No exceptions.

## Error Handling

- `RecoverableError` for expected, input-driven failures → `isError: false` (sibling calls survive)
- `anyhow::bail!` for genuine tool failures → `isError: true` (fatal)
- Write tools return `json!("ok")` — never echo content back. Reserve richer responses only for genuinely new info (e.g. LSP diagnostics after a write).
- See `get_guide("error-handling")` for the full decision tree

## MCP Entry Point

`call_content()` is the MCP entry point — it handles buffer routing via `OutputGuard`.
Do NOT call `call()` directly from `ServerHandler`; it bypasses buffer routing.

## Progressive Disclosure

Every tool defaults to compact/exploring output. Full bodies only with `detail_level: "full"`.
Two modes via `OutputGuard` (`src/tools/output.rs`): Exploring (compact, capped at 200 items) / Focused (full detail, paginated).
Overflow → actionable hint + `by_file` distribution map, never truncated garbage.
See `docs/PROGRESSIVE_DISCOVERABILITY.md` before adding or modifying any tool.

## Agent-Agnostic Design

codescout serves multiple agents (Claude Code, Copilot, Gemini, Antigravity). The server must be **self-contained** — its gate logic, error messages, and instructions must guide any MCP client to the right tool without relying on external hooks:

- Error hints name codescout tools (`edit_code`, `grep`), never host tools (`Edit`, `Write`) — the LLM must never be nudged to sidestep codescout via native file editing.
- The companion plugin (`codescout-companion`) adds Claude-Code-specific enforcement (PreToolUse hooks), but the server itself must not depend on it.
- Project workflows, standards, and artifacts live in the repo (`docs/…`, `CLAUDE.md`), NOT in `claude-plugins/`. Plugin content is a thin UX wrapper over repo-resident source of truth — a non-CC client must never be locked out. When in doubt: would a Copilot user lose access? Then it belongs in the repo.

## Testing Patterns

- Cache-invalidation tests use a **three-query sandwich** (baseline → assert-stale → invalidate → assert-fresh), not two. The step-3 stale assertion is what makes it a *regression* test — it fails if the system ever changes to eager-reread. Canonical example: `did_change_refreshes_stale_symbol_positions` in `src/lsp/client.rs`.
- Test helpers that build env-reading objects (resolve `LIBRARIAN_DB`/`LIBRARIAN_WORKSPACE`/etc. from process-global env) must return an `EnvGuard` and the calling test must carry `#[serial_test::serial]`. See `docs/conventions/test-env-isolation.md`.
- Fallback-path tests gated on an exact-match miss must avoid substring overlap in fixtures (else the exact path fires first → false green). Assert on a path-specific marker so a mis-route fails loudly. See `docs/trackers/reconnaissance-patterns.md` R-16.

## Prompt Surface Consistency

Any tool rename, addition, or behavior change requires updating all three prompt surfaces.
The build-time test `prompt_surfaces_reference_only_real_tools` catches stale tool names; `claude_md_contains_no_deprecated_tool_names` guards `CLAUDE.md`.
Bump `ONBOARDING_VERSION` only for `onboarding_prompt` surface changes — never for `server_instructions`.
Full operational detail (bump matrix, 2200-byte slice cap, verify-slice hazard) + the writing style guide live in `src/prompts/README.md`.

## Bug Tracking

Every noticed bug gets a file in `docs/issues/YYYY-MM-DD-<slug>.md` (copy `_TEMPLATE.md`).
Archive to `docs/issues/archive/` only after the fix ships to `master` (verify via `git branch --contains`).
Frontmatter/status vocabulary: `get_guide("tracker-conventions")`.

## Commit Style

Conventional commits: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`.
Subject: imperative, ≤72 chars. Cherry-pick to master after all checks pass + MCP verify.
Full release + ship procedures: `docs/RELEASE.md`. SHA-citation + cross-repo-prefix discipline: memory `gotchas`.
