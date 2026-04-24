# Workspace Conventions

## Commit Style

- Conventional commits: `feat(scope): ...`, `fix(scope): ...`, `chore: ...`, `docs: ...`
- Scope = crate name or subsystem: `onboarding`, `lsp`, `embed`, `librarian`, `review/date`
- Single well-tested commit per fix/feature — batch related changes, don't commit intermediates
- Co-author line: `Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>` on AI-assisted commits

## Branch Strategy

- `master` — protected, production-quality only; cherry-picked commits from `experiments`
- `experiments` — all experimental/in-progress work; iterate freely here
- Feature branches for large parallel tracks if needed
- Never commit directly to master; never force-push master

## PR / Ship Process

1. Work on `experiments`; verify: `cargo fmt && cargo clippy -- -D warnings && cargo test`
2. For tools: also `cargo build --release` + `/mcp` restart + manual MCP verification
3. Cherry-pick clean commit to master: `git cherry-pick <sha>`
4. Push master; rebase experiments: `git checkout experiments && git rebase master`

## Experimental Feature Docs

When adding a feature commit to `experiments`:
- Create `docs/manual/src/experimental/<feature-name>.md` with `> ⚠ Experimental` callout
- Add entry to `docs/manual/src/experimental/index.md`
- On graduation to master: `git mv` doc to target chapter, remove callout, update SUMMARY.md

## CI Rules (enforced pre-commit)

- `cargo fmt` — no formatting diffs
- `cargo clippy -- -D warnings` — zero warnings
- `cargo test` — all tests pass (1142 tests: 1110 unit + 10 integration + 22 symbol_lsp)
- `panic = "abort"` in release profile (Cargo.toml) — no zombie server processes

## Error Handling (code-explorer)

- `RecoverableError` for expected input-driven failures → `isError: false` (sibling calls survive)
- `anyhow::bail!` for genuine tool failures → `isError: true` (fatal)
- Write tools return `json!("ok")` — never echo content back

## Tool Development Rules

When adding a new tool: update 6 locations (struct, server registration, test list, path_security
check_tool_access, disabled-blocks test, server_instructions.md).

When renaming a tool: update all 3 prompt surfaces (server_instructions.md, onboarding_prompt.md,
builders.rs) and bump ONBOARDING_VERSION in src/tools/onboarding.rs.

## Per-Project Specifics

- **code-explorer**: see CLAUDE.md § Prompt Surface Consistency; Tool trait at `src/tools/mod.rs:543`
- **librarian-mcp**: simpler Tool trait (no call_content/OutputGuard); parking_lot::Mutex for catalog
- **codescout-embed**: feature-gated compilation (local-embed / remote-embed); no tool trait
- **fixture libraries**: read-only test targets; never add external dependencies
