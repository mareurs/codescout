---
id: e4fbea5400c81bc4
kind: bug
status: fixed
title: 'BUG: IL-3''s grep remedy hint suggests symbol tools for identifier-shaped patterns that are plain string literals, never mentioning grep()'
tags:
- cluster/hint-composed-without-the-request
- il3
- run_command
- path_security
closed: 2026-09-14
---

## Summary
When `run_command`'s IL-3 gate blocks a shell `grep` whose pattern is an alternation of identifier-shaped tokens (e.g. `FOO|BAR|BAZ`), `check_source_file_access`'s remedy branch assumes the caller wants a *symbol* lookup and suggests only `symbols`/`references`/`call_graph` — tools that search the AST/LSP index, not text. It never mentions `grep(pattern, path)`, the codescout MCP tool that actually performs a full-text search and is unaffected by IL-3. When the tokens are plain string literals (e.g. env var names passed to `std::env::var(...)`) rather than declared symbols, the suggested tools return zero results and the remedy is unperformable for the caller's actual request.

## Symptom (Effect)
```
run_command(command: "grep -rn \"CODESCOUT_EMBEDDER_MODEL_NAME\\|EMBED_API_KEY\\|CODESCOUT_MODEL_DIM\\|AZURE_OPENAI\\|EMBED_API_BASE\\|EMBED_ENDPOINT\\|EMBED_BASE_URL\" <project> --include=*.rs -l")
{
  "ok": false,
  "error": "shell access to source files is blocked",
  "hint": "use symbols(name='CODESCOUT_EMBEDDER_MODEL_NAME') for declarations, references(symbol='CODESCOUT_EMBEDDER_MODEL_NAME') for direct callers, call_graph(symbol='CODESCOUT_EMBEDDER_MODEL_NAME', direction='callers') for transitive blast radius. Re-run with acknowledge_risk: true if you need raw shell grep."
}
```
`symbols(name='CODESCOUT_EMBEDDER_MODEL_NAME')` (the suggested remedy) returns **0 matches** — it is not a declared Rust symbol, just a string literal argument. `grep(pattern='CODESCOUT_EMBEDDER_MODEL_NAME|EMBED_API_KEY|CODESCOUT_MODEL_DIM', glob='*.rs', mode='files')` — the codescout MCP tool, never mentioned in the hint — returns **50 matches across 13 files** immediately, with no gate involvement at all.

## Reproduction
1. In this repo, `symbols(name="CODESCOUT_EMBEDDER_MODEL_NAME")` → 0 matches.
2. `grep(pattern="CODESCOUT_EMBEDDER_MODEL_NAME|EMBED_API_KEY|CODESCOUT_MODEL_DIM", glob="*.rs", mode="files")` → 50 matches, 13 files.
3. `run_command(command="grep -rn 'CODESCOUT_EMBEDDER_MODEL_NAME|EMBED_API_KEY' . --include=*.rs -l")` → blocked, hint suggests only `symbols`/`references`/`call_graph`.

git commit at time of filing: HEAD of `experiments`, `3396baae` and later (see `git log`).

## Environment
Rust MCP server, `run_command` shell gate (`src/util/path_security.rs`), any project. Reproduced on Linux; the originating report was from a session on a Windows/git-bash checkout (path style `/c/Users/...`) — irrelevant to the mechanism, which is pattern-shape-only.

## Root cause
`check_source_file_access`'s remedy-selection (`src/util/path_security.rs`, the `"grep" => { if is_identifier_pattern(&pat) { ... } else { ... } }` arm) picks the remedy from the **shape of the extracted pattern alone**: `is_identifier_pattern` only checks that every `|`-separated part is `[A-Za-z_][A-Za-z0-9_]*`. It does not — and structurally cannot, since it has no index access at that point — check whether any of those tokens is an actual declared symbol vs. a string literal. An alternation of several ALL-CAPS env-var-style names is exactly the shape that satisfies `is_identifier_pattern` while being the *least* likely case to be a symbol lookup (an alternation implies "any of these," which is a text/content search, not "where is this one thing declared"). The `else` branch (non-identifier-shaped pattern) does correctly suggest `grep(pattern, path)` — the identifier-shaped branch just never offers it. inferred from `src/util/path_security.rs` (the `check_source_file_access` remedy `match` block) — not measured beyond the two tool calls in Reproduction.

Fits `cluster/hint-composed-without-the-request` (IC-22, `docs/trackers/issue-clusters.md`): the hint is composed from the response's own shape (the pattern's lexical form) rather than from what the request was actually for (search text content across files vs. look up one symbol's declaration).

## Evidence
### `symbols()` finds nothing for the suggested remedy
```
symbols(name="CODESCOUT_EMBEDDER_MODEL_NAME") → 0 matches
```
### `grep()` — never mentioned in the hint — finds it immediately
```
grep(pattern="CODESCOUT_EMBEDDER_MODEL_NAME|EMBED_API_KEY|CODESCOUT_MODEL_DIM", glob="*.rs", mode="files")
→ 50 matches in 13 files: src/retrieval/embedder.rs, crates/codescout-embed/src/remote.rs,
  src/retrieval/config.rs, src/retrieval/client.rs, tests/retrieval_unit.rs, src/retrieval/search.rs,
  tests/embedder_env_isolation.rs, src/agent/mod.rs, src/cli/doc.rs, src/config/project.rs,
  src/librarian/mod.rs, src/tools/semantic/semantic_search.rs, tests/env_mutation_isolation.rs
```

## Hypotheses tried
1. **Hypothesis:** the shell-gate block itself was wrong (path misclassified as in-project). **Test:** read `check_source_file_access` → `segment_reads_project_source` → `path_is_within_project`; the absolute-path branch is a bare `expanded.starts_with(project_root)`, no heuristic. **Verdict:** rejected — the block was correct given the path really was under the project root. **Evidence link:** none needed beyond source read (see `bug-fix-session-log:W-133`, which is where this was first (incorrectly) closed out as "hint was correct").
2. **Hypothesis:** the *remedy* text, not the block, is what's wrong for this pattern class. **Test:** ran the suggested remedy (`symbols`) and the omitted alternative (`grep`) side by side. **Verdict:** confirmed. **Evidence link:** Evidence section above.

## Fix
Resolved by removing `grep` command-wide from `SOURCE_ACCESS_COMMANDS` (`src/util/path_security.rs`) rather than by fixing this remedy's text — the whole `"grep" => { ... }` arm this bug is about, along with `extract_grep_pattern`, is now dead code and has been deleted. `grep` no longer routes through `check_source_file_access` at all, so there is no remedy branch left to misfire. `is_identifier_pattern` is kept (live caller in `src/tools/grep.rs`).

Fix commit: `439cd82f6a874267f771284954d983720c3ad5fa` on `experiments`.
Patch-id: `e2530335cdc68746233803ff8fe87468bd8328f5` (`git show 439cd82f | git patch-id --stable`).

Status: **fixed** — moot by removal, not a text fix.
## Tests added
`util::path_security::tests::grep_on_in_project_source_is_no_longer_blocked` (`src/util/path_security.rs`) — asserts shell `grep` on an in-project source file, and a recursive `--include=*.rs` grep, are both now allowed. The six tests that pinned the old (buggy) remedy text were deleted, since their premise (the `"grep" =>` arm) no longer exists. Verified: `cargo test --workspace --no-default-features ; cargo test --workspace`, both green for this change (one unrelated pre-existing failure elsewhere, in `librarian::tools::update_entry`, tracked separately).
## Workarounds
Use the codescout `grep()` MCP tool directly (`grep(pattern=..., glob=..., mode="files")`) instead of following the auto-generated hint literally — it is not blocked by IL-3 at all and was the correct tool the whole time. Raw shell grep remains available via `acknowledge_risk: true` if truly needed.

## Resume
N/A — fixed and verified on `experiments`.
## References
- `docs/trackers/bug-fix-session-log.md` — `W-133` (initial, incomplete verification; corrected by an F-N entry filed alongside this bug)
- `docs/trackers/issue-clusters.md` — `IC-22`, slug `hint-composed-without-the-request`
- `src/util/path_security.rs` — `check_source_file_access`, `segment_reads_project_source`, `path_is_within_project`, `is_identifier_pattern`
