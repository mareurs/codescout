---
id: null
kind: bug
status: fixed
title: null
owners: []
tags:
- lsp
- workspace-pinning
- references
- symbol_at
- call_graph
topic: null
time_scope: null
closed: 2026-06-14
---

# BUG: references / symbol_at / call_graph ignore the workspace= pin for relative path resolution

## Summary
The LSP-position tools `references`, `symbol_at`, and `call_graph` resolve their
relative `path` argument against the **session-default** active project, not the
per-request `workspace=` pin. A subagent pinned to a foreign workspace that passes a
project-relative path gets `path not found` (or, worse, could silently resolve to a
same-named file in the wrong project). `symbols` (overview + search) and `read_file`
already honor the pin; these three tools were missed when per-request pinning was
propagated.

## Symptom (Effect)
Calling `references` pinned to a foreign workspace with a project-relative path:

```
references(symbol="CalendarService",
           path="ktor-server/src/main/kotlin/edu/planner/service/scheduling/CalendarService.kt",
           workspace="/home/marius/work/mirela/backend-kotlin")
→ { "ok": false,
    "error": "path not found: ktor-server/src/main/kotlin/edu/planner/service/scheduling/CalendarService.kt",
    "hint": "Use tree to explore the directory structure, or symbols(path) to list symbols in a file or directory." }
```

The identical relative path resolves fine via `symbols(path=..., workspace=...)`.
Passing the **absolute** path to `references` resolves (absolute paths bypass the
project-root join), which localizes the defect to root selection, not the path string.

## Reproduction
- git HEAD: `1e8b9eb1c01b53ac9b218af57ba4cfe9f1ef73de` (branch `experiments`).
- Active project = codescout (any project whose root differs from the pin target).

1. From a session whose active project is A, call:
   `references(symbol=<S>, path=<relative-path-valid-in-B>, workspace=<abs path of B>)`
2. Observe `path not found` — the relative path was joined onto A's root, not B's.
3. `symbols(path=<same relative path>, workspace=<B>)` succeeds (control).
4. Re-issue references with the **absolute** path under B → resolves.

Observed 4×: three parallel subagents pinned to backend-kotlin worktrees + one direct
controller call against the warm live backend-kotlin mux (`26a9e85d58931839`).

## Environment
Linux; codescout v0.15.0 (release, `experiments`); MCP stdio. Reproduced both warm
(100-min-old live mux) and cold (fresh worktree muxes) — independent of LSP state.

## Root cause
`src/tools/symbol/references.rs:143` resolves the path with the **unpinned** resolver:

```rust
let full_path = resolve_read_path(&ctx.agent, rel_path).await?;
```

`src/fs/mod.rs:44-51` documents `resolve_read_path` as resolving "against the
session-default project. For per-request `workspace=` pinning, use
`resolve_read_path_for`." The `require_project_root_for(ctx.workspace_override)`
call on the *next* line (`references.rs:146`) only feeds downstream tagging — the
path was already joined onto the session-default root.

Same defect, same unpinned call, in:
- `src/tools/symbol/symbol_at.rs:74` and `:202`
- `src/tools/symbol/call_graph/mod.rs:402`

Correct (pinned) pattern, for contrast — `src/tools/symbol/list_overview.rs:277`:

```rust
let full_path =
    resolve_read_path_for(&ctx.agent, ctx.workspace_override.as_deref(), rel_path).await?;
```

`read_file` was fixed the same way previously (`src/fs/mod.rs` comment: "the pinning
gap that `read_file` already closed").

## Root cause — defect #2: LSP root ignores the pin (silent, more severe)

The pin gap exists at a **second, deeper, more severe layer**: the LSP root.

`get_lsp_client` (`src/fs/mod.rs:284`) resolves the LSP workspace root via the
**unpinned** `agent.require_project_root()` (`:297`), NOT
`require_project_root_for(workspace_override)` — it doesn't even take a
`workspace_override` parameter. So `lsp.get_or_start(lang, root, ...)` is keyed
on the **active project**, never the pinned workspace.

Consequence: a source file pinned to a foreign workspace is opened in the
**active project's** LSP for that language. A kotlin file pinned to a foreign
kotlin workspace while the active project is codescout connects to codescout's
own kotlin mux (`7e868829`) and queries it for a file it never indexed →
silently returns 0 references (or wrong defs/hovers). Unlike defect #1 (path
resolution → loud `path not found`), this is **SILENT** — plausible-but-wrong
results.

**Confirmed empirically (2026-06-11):** kotlin calls pinned to fresh
backend-kotlin worktrees (cs-probe, cs-fix4) touched codescout's kotlin home
`7e868829`, never the pinned worktree's home, and never spawned a worktree mux.
This also explains the F-18 "no mux for a fresh worktree" behavior in
`bug-fix-session-log.md` — it is this routing, not poisoning or cold-start
failure.

**Fix scope (9 call sites + helper):** thread `workspace_override` into
`get_lsp_client` and use `require_project_root_for(...)` at `fs/mod.rs:297`;
update callers `references.rs:151`, `symbol_at.rs:82`+`:207`,
`call_graph/mod.rs:404`, `list_overview.rs:287`, `edit_code.rs:170/443/511/746`,
and the internal call in `retry_on_mux_disconnect` (`fs/mod.rs:341`). `edit_code`
is included → pinned **writes** also currently route to the active project's LSP.

**Must ship with defect #1.** Defect #1's fix alone makes the path resolve but
leaves the LSP query mis-routed (arguably worse — looks like it works). Both
together = correct per-request pinned LSP behavior. Blocks the Fix-4 contention
repro: a fresh-workspace mux can't be triggered via tool calls until this lands.

**Severity:** high (silent wrong-LSP routing on every pinned LSP op, incl. writes).
## Evidence
### Warm-mux controller reproduction (this session)
Relative path → `path not found` (quoted above). Absolute path under the same pin →
resolved (returned the definition site, with the kotlin-lsp reference-completeness
warning — a separate, already-handled concern).

### Parallel-subagent reproduction
3/3 stress-test subagents pinned to backend-kotlin worktrees hit `path not found` on
the relative path; 2/3 worked around it with the absolute path.

### Code divergence
grep over `resolve_read_path` (unpinned) vs `resolve_read_path_for` (pinned):
`references.rs:143`, `symbol_at.rs:74/202`, `call_graph/mod.rs:402` use the unpinned
variant; `list_overview.rs:277` uses the pinned variant. (`symbols` search resolves
its root via `require_project_root_for(workspace_override)` and is unaffected.)

## Hypotheses tried
1. **Hypothesis:** cold-start / index-warming caused the failure.
   **Test:** repeated the relative-path call against the 100-min-warm live mux.
   **Verdict:** rejected — warm mux returns the same `path not found`.
2. **Hypothesis:** the wrong-language LSP (rust-analyzer) was being selected for the
   Kotlin worktree.
   **Test:** `ls` the worktree → found `eduplanner-mcp/Cargo.toml`.
   **Verdict:** rejected — backend-kotlin is polyglot; the rust-analyzer muxes are
   correct detection of the Rust subproject, unrelated to this path bug.
3. **Hypothesis:** path resolution ignores the pin (resolves against active project).
   **Test:** absolute path resolves; relative fails; code uses unpinned
   `resolve_read_path`.
   **Verdict:** confirmed — root cause.

## Fix

**Shipped on `experiments` in `85dc92f9`** (`fix(symbols): honor workspace= pin in references/symbol_at/call_graph/edit_code`). **Verified fixed 2026-06-14** (verify-open scout for the "solve all open bugs" pass): both call-site swaps to `resolve_read_path_for` and the `get_lsp_client` LSP-root pin are present in current code; all 7 `honors_workspace_override` tests green. Not yet on `master` — archive to `docs/issues/archive/` after cherry-pick and cite the master-side SHA then.

**Defect #1 (path resolution):** swapped the 4 call sites to
`resolve_read_path_for(&ctx.agent, ctx.workspace_override.as_deref(), …)` —
`references.rs:143`, `symbol_at.rs:74/202`, `call_graph/mod.rs:402`. Removed the
now-dead `resolve_read_path` wrapper from `fs/mod.rs` + fixed its doc/test caller.

**Defect #2 (LSP root):** added `workspace_override: Option<&Path>` to
`get_lsp_client` (`fs/mod.rs`); it now resolves the LSP root via
`require_project_root_for(workspace_override)` instead of the unpinned
`require_project_root()`. Threaded the pin through `retry_on_mux_disconnect` and
all 9 `get_lsp_client` call sites (`references.rs`, `symbol_at.rs`×2,
`call_graph/mod.rs`, `list_overview.rs`, `edit_code.rs`×4) + the 4
`retry_on_mux_disconnect` callers.

Gates: `cargo fmt` + `clippy --all-targets -D warnings` clean; pin tests pass.
Release built. Defect #1 verified live (relative path + foreign pin now resolves).
Defect #2 live verification pending a `/mcp` reconnect + the Fix-4 contention repro
(which is unblocked once defect #2 is live). Master-side SHA: TBD (cite after cherry-pick).
## Tests added

- `references_honors_workspace_override_for_relative_path`,
  `symbol_at_honors_workspace_override_for_relative_path`,
  `call_graph_honors_workspace_override_for_relative_path` (`src/tools/symbol/tests.rs`)
  — defect #1: a relative path pinned to a foreign workspace resolves past the
  path stage (asserts the error is `unsupported language`/`unsupported file type`,
  never `path not found`).
- `get_lsp_client_honors_workspace_override_for_lsp_root` (`src/fs/mod.rs`)
  — defect #2: a recording `LspProvider` confirms `get_lsp_client` passes the
  PINNED root to `get_or_start`, not the active project (the silent bug made loud).
## Workarounds
Pass an **absolute** `path` to `references` / `symbol_at` / `call_graph` when using
`workspace=`. Absolute paths bypass the project-root join entirely.

## Resume
Implement the 4-site swap to `resolve_read_path_for` listed in `## Fix`; add three
per-tool regression tests; `cargo fmt` / `clippy -D warnings` / `test`; build release
+ `/mcp` restart; then re-run
`references(symbol=..., path=<relative>, workspace=<foreign abs path>)` and confirm it
resolves. Empirically confirm `symbol_at` and `call_graph` too — only `references` was
exercised live this session (the other two are confirmed by identical code).

## References
- `src/fs/mod.rs:44-62` — resolver doc comment + `resolve_read_path_for`
- `src/tools/symbol/list_overview.rs:277` — correct pinned pattern
- Sibling per-request pinning work, same day: commit `9fa4d482` ("overview honors workspace= pin")
- Surfaced during the 3-worktree concurrent kotlin-lsp stress test, 2026-06-11
