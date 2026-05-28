# codescout — Conventions

## Pre-Commit Requirements

1. `cargo fmt`
2. `cargo clippy -- -D warnings`
3. `cargo test`
All three must pass. No exceptions.

## Error Handling

- `RecoverableError` for expected, input-driven failures → `isError: false` (sibling calls survive)
- `anyhow::bail!` for genuine tool failures → `isError: true` (fatal)
- Write tools return `json!("ok")` — never echo content back
- See `get_guide("error-handling")` for the full decision tree

## MCP Entry Point

`call_content()` is the MCP entry point — it handles buffer routing via `OutputGuard`.
Do NOT call `call()` directly from `ServerHandler`; it bypasses buffer routing.

## Progressive Disclosure

Every tool defaults to compact/exploring output. Full bodies only with `detail_level: "full"`.
Overflow → actionable hint + `by_file` distribution map, never truncated garbage.
See `docs/PROGRESSIVE_DISCOVERABILITY.md` before adding or modifying any tool.

## Testing Patterns

- Cache-invalidation tests use a **three-query sandwich** (baseline → stale assertion → post-invalidation)
- Test helpers that build env-reading objects must use `EnvGuard` + `#[serial_test::serial]`
- See `docs/conventions/test-env-isolation.md` for the full isolation rule

## Prompt Surface Consistency

Any tool rename, addition, or behavior change requires updating all three prompt surfaces.
The build-time test `prompt_surfaces_reference_only_real_tools` catches stale tool names.
Bump `ONBOARDING_VERSION` only for `onboarding_prompt` surface changes — never for `server_instructions`.

## Bug Tracking

Every noticed bug gets a file in `docs/issues/YYYY-MM-DD-<slug>.md` (copy `_TEMPLATE.md`).
Archive to `docs/issues/archive/` only after the fix ships to `master` (verify via `git branch --contains`).

## Commit Style

Conventional commits: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`.
Subject: imperative, ≤72 chars. Cherry-pick to master after all checks pass + MCP verify.