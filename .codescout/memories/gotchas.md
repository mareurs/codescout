# Workspace Gotchas

## Semantic Index — Fixture Projects Not Indexed

The semantic index is populated for `code-explorer` only. All fixture projects
(java-library, kotlin-library, python-library, rust-library, typescript-library,
nav-eval-rust, edit-eval-rust) have no separate semantic index.
**When searching within fixture projects:** skip `semantic_search`; use
`grep(pattern, path="tests/fixtures/<name>/src")` or `symbols(path="tests/fixtures/<name>/")` directly.

## Kotlin LSP Circuit-Breaker

`kotlin-language-server` circuit-breaker trips when two codescout instances target the same
Kotlin project concurrently. `symbols(include_body=true)` will fail with "circuit-breaker open".
**Workaround:** use `grep` as fallback.
See `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md`.

## eval Fixture Workspace Isolation

`edit-eval-rust` and `nav-eval-rust` declare their own `[workspace]` tables and must
**never** be added as workspace members of code-explorer. Their `Cargo.lock` must stay separate.
`git restore tests/fixtures/edit-eval-rust/src` resets mutations — all `src/` files must be
git-tracked or restore silently no-ops and mutations leak between eval cases.

## MCP Binary Symlink

`~/.cargo/bin/codescout` is a symlink → `target/release/codescout`.
`cargo build` (dev profile) does NOT update the live binary. Only `cargo build --release` does.
After a release build, run `/mcp` to reconnect. If the symlink is missing after `cargo clean`,
recreate: `ln -sf "$(pwd)/target/release/codescout" ~/.cargo/bin/codescout`.

## RemoteEmbedder Dimensions

`RemoteEmbedder.dimensions()` returns `0` until after the first successful `embed()` call
(uses `AtomicUsize` cached lazily). Callers needing a guaranteed non-zero dimension must
embed a sample text first.

## Cherry-Pick SHA Discipline

Always record the **master-side SHA** after cherry-pick, not the experiments-side original.
After `git rebase master`, experiments-side originals become orphans — `git branch --contains`
returns empty. Use `git log master --oneline --grep="<subject>"` to recover master SHA if needed.

## Cross-Repo Commit References

When a tracker cites a commit from a sibling repo, prefix: `<repo>:<sha>` (e.g. `codescout-companion:0b75991`).
A bare SHA implies the current repo. Unenforced by tooling — readers must notice the prefix.

## Onboarding Subagent Project-Scope Collision

During parallel force-onboarding, subagents may overwrite each other's memories in the
`code-explorer` project slot (last writer wins when multiple subagents share the focused project).
Verify `memory(action="read", project_id="code-explorer", topic="project-overview")` after onboarding
to confirm the content is actually about codescout and not a fixture crate.