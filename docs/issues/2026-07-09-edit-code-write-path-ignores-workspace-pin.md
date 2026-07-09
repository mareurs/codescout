---
id: cdadfba57082cb14
kind: bug
status: fixed
title: 'BUG: edit_code write-path resolution ignores the workspace= pin — structural edits land in the session-default project, not the pinned workspace'
owners:
- marius
tags:
- edit_code
- workspace-pin
- regression
- write-path
topic: null
time_scope: null
closed: '2026-07-09'
opened: '2026-07-09'
severity: high
---

# BUG: edit_code write-path resolution ignores the workspace= pin — structural edits land in the session-default project, not the pinned workspace

## Summary
All four `edit_code` actions (`insert`, `replace`, `remove`, `rename`) resolve
the file they actually read and write via `resolve_write_path`, which always
joins the caller's relative `path` onto the **session-default** active
project — never the per-request `workspace=` pin. A subagent pinned to a
worktree (or any foreign workspace) whose `path` also exists at the same
relative location in the session-default project gets its structural edit
silently applied to the **wrong file** — the default project's copy, not the
pinned workspace's. No error is raised; the LSP symbol lookup succeeds
(against whichever file `full_path` happens to point at), so the tool call
reports `"status": "ok"`.

Reported symptom (external, from an SDD execution against backend-kotlin):
four implementer-subagent dispatches, each pinned via `workspace=<worktree
abs path>`, had their `edit_code` structural inserts land in the main repo
instead of the worktree. Each was self-caught via `git status` and reverted;
no lasting damage, but 100% reproducible per the mechanism below.

## Symptom (Effect)
A regression test (`edit_code_insert_honors_workspace_override_pin`,
`src/tools/symbol/tests.rs:6969`) pins workspace A ("worktree") and calls
`edit_code(action="insert", path="src/lib.rs", workspace=<A>)` while the
session-default project is workspace B ("main repo"), with both containing
an identical `src/lib.rs`. Observed (before the fix):

```
thread 'tools::symbol::tests::edit_code_insert_honors_workspace_override_pin' panicked at src/tools/symbol/tests.rs:7040:5:
insert pinned to workspace A must land in A's file, not B's; A content:
pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

---
B content:
pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}
pub fn pinned_insert_marker() {}
```

The tool call itself returned `Ok` (`"status": "ok"`) — no error surfaces to
the caller. Workspace A (the pin target) is untouched; workspace B (the
session default) received the insert.

## Reproduction
- git HEAD: `a9062a26dffe0e0c224eb98d1001df644398d1b6` (branch `experiments`).
- `cargo test --lib edit_code_insert_honors_workspace_override_pin -- --nocapture`
  (requires `rust-analyzer` on PATH; skips cleanly if absent).

Manual/live repro:
1. Activate project A as the session default (`workspace(activate, path=A)`).
2. Call `edit_code(action="insert", path="<rel>", symbol=..., body=...,
   workspace="<B abs path>")` where `<rel>` exists in both A and B.
3. Before the fix: B's file is unchanged; A's file (the session default, not
   the pin) received the edit. Inverted from the caller's intent.

## Environment
Rust/tokio async; codescout MCP server; any transport. Affected
`edit_code`'s `insert`/`replace`/`remove`/`rename` actions uniformly — same
defective call at the top of each `do_*` method. Not language-specific;
reproduced here with Rust + rust-analyzer, originally observed with Kotlin
(backend-kotlin, `~/work/mirela/backend-kotlin`, multi-worktree SDD
execution).

## Root cause
`resolve_write_path` (`src/fs/mod.rs:76-84`, pre-fix) had no
`workspace_override` parameter at all:

```rust
pub(crate) async fn resolve_write_path(
    agent: &Agent,
    relative_path: &str,
) -> anyhow::Result<PathBuf> {
    let root = agent.require_project_root().await?;
    let security = agent.security_config().await;
    let session_roots = agent.session_write_roots_snapshot().await;
    crate::util::path_security::validate_write_path(relative_path, &root, &security, &session_roots)
}
```

`Agent::require_project_root()` (`src/agent/mod.rs:824-843`) unconditionally
resolves `inner.default_workspace()` — the session's active/default project.
It had no path to ever consult a per-call pin.

All four `EditCode::do_*` methods (`src/tools/symbol/edit_code.rs`) called
this as the **first line**, before `ctx.workspace_override` was consulted
anywhere:

```rust
let full_path = resolve_write_path(&ctx.agent, rel_path).await?;   // <- always session-default root
```

- `do_rename`  — `edit_code.rs:168` (pre-fix)
- `do_remove`  — `edit_code.rs:447` (pre-fix)
- `do_replace` — `edit_code.rs:522` (pre-fix)
- `do_insert`  — `edit_code.rs:762` (pre-fix)

Every *subsequent* call in the same methods correctly threaded the pin —
`get_lsp_client(..., ctx.workspace_override.as_deref())`,
`invalidate_call_edges_for(ctx.workspace_override.as_deref(), ...)`,
`mark_file_dirty_for(ctx.workspace_override.as_deref(), ...)`, and (in
`do_rename`'s second phase) `require_project_root_for` /
`security_config_for` / `session_write_roots_snapshot_for`. But by the time
any of those ran, `full_path` — the one thing that determines which file is
actually read (`std::fs::read_to_string`) and written
(`atomic_write`/`write_lines`) — was already bound to the wrong root. The pin
was honored for LSP routing and cache invalidation, but not for the file I/O
that matters.

This is a **sibling defect** to the one fixed under catalog id
`3fb29bc678a32562` ("references / symbol_at / call_graph ignore the
workspace= pin"), shipped `85dc92f9`. That fix covered:
1. Defect #1 — unpinned *read*-path resolution (`resolve_read_path` →
   `resolve_read_path_for`) in `references.rs`/`symbol_at.rs`/`call_graph/mod.rs`.
2. Defect #2 — unpinned LSP-root resolution in `get_lsp_client`, fixed by
   adding a `workspace_override` parameter, threaded through **9 call
   sites including `edit_code.rs`'s 4** (confirmed present pre-fix — this is
   why `get_lsp_client` calls inside `edit_code.rs` already showed
   `ctx.workspace_override.as_deref()` even before this session's fix).

Defect #2's fix did NOT touch `resolve_write_path` — it was out of scope
(that bug's fix list explicitly enumerates `get_lsp_client` call sites, not
the write-path resolver). `edit_code` was the only tool that called
`resolve_write_path`; no other write-path helper in the codebase shared this
gap — `edit_file` resolves its write path via a different function,
`resolve_write_or_capture` (`src/tools/core/write_ack.rs:78-126`), which
*does* correctly call `require_project_root_for(ctx.workspace_override.as_deref())`,
`security_config_for(...)`, and `session_write_roots_snapshot_for(...)`. That
divergence — two different "resolve a path for writing" implementations, one
pin-aware and one not — is why `edit_file` was not implicated in the
original report and `edit_code` was.

## Evidence
### Regression test (this session, pre-fix)
`src/tools/symbol/tests.rs:6969` —
`edit_code_insert_honors_workspace_override_pin`. Failed on pre-fix code;
panic output quoted above shows the insert landed in workspace B (session
default) instead of workspace A (the pin target). Passes post-fix.

### Code inspection
- `src/fs/mod.rs:76-84` (pre-fix) — `resolve_write_path`, no override parameter.
- `src/agent/mod.rs:824-843` — `Agent::require_project_root`, unconditionally
  session-default.
- `src/agent/mod.rs:636-642` + `586-613` — `require_project_root_for` /
  `with_project_at`, the pin-aware equivalents that existed and were used
  elsewhere but not by `resolve_write_path`.
- `src/tools/symbol/edit_code.rs:168,447,522,762` (pre-fix) — the four
  unpinned call sites.
- `src/tools/core/write_ack.rs:78-126` — the correct, already-pinned pattern
  used by `edit_file`, for contrast.

## Hypotheses tried
1. **Hypothesis:** the bug is in `get_lsp_client`'s root resolution (same as
   the previously-fixed defect #2).
   **Test:** read `get_lsp_client` (`src/fs/mod.rs`) and all 4 `edit_code.rs`
   call sites.
   **Verdict:** rejected — `ctx.workspace_override.as_deref()` was already
   correctly passed to `get_lsp_client` at all 4 sites; that part of the
   prior fix was intact.
2. **Hypothesis:** `full_path` (used for the actual file read/write) is
   resolved before the pin is ever consulted, via a resolver with no
   override parameter.
   **Test:** traced `resolve_write_path` → `Agent::require_project_root()`;
   compared against the pin-aware `require_project_root_for` used by
   `do_rename`'s second phase and by `edit_file`'s
   `resolve_write_or_capture`.
   **Verdict:** confirmed — root cause. Backed by the (then-)failing
   regression test.

## Fix

Implemented on `experiments` (not yet committed as of this writing). Mirrors the already-shipped `85dc92f9` pattern exactly:

- `resolve_write_path` (`src/fs/mod.rs:76-84`, no override param) was replaced in place with `resolve_write_path_for(agent, workspace_override, relative_path)` (`src/fs/mod.rs:78-89`), which resolves `root`/`security`/`session_roots` via `require_project_root_for`/`security_config_for`/`session_write_roots_snapshot_for` instead of the unpinned equivalents — then renamed with `edit_code(action="rename")` to pick up the `_for` suffix already established by `resolve_read_path_for`.
- All 4 call sites in `src/tools/symbol/edit_code.rs` updated to pass `ctx.workspace_override.as_deref()` as the new second argument: `do_rename:169`, `do_remove:449`, `do_replace:525`, `do_insert:766`.
- Confirmed via `grep` that `resolve_write_path`/`resolve_write_path_for` has exactly these 4 call sites in live source — no other caller needed migration.

Gates: `cargo fmt --check` clean, `cargo clippy --all-targets -- -D warnings` clean, `cargo test --lib` → 2961 passed, 0 failed, 6 ignored (unrelated, pre-existing). `cargo rb` release build verified. Master-side SHA: N/A (not yet committed/cherry-picked).

## Tests added

- `resolve_write_path_for_honors_workspace_override` — `src/fs/mod.rs:495` (new, fast, no LSP required). Direct unit test on the resolver itself: unpinned resolution joins onto the session-default workspace B, pinned resolution (`Some(root_a)`) joins onto workspace A. Mirrors `resolve_read_path_for_honors_workspace_override` (`src/fs/mod.rs:461`).
- `edit_code_insert_honors_workspace_override_pin` — `src/tools/symbol/tests.rs:6969` (added this session; now passes with the fix). Live, tool-level, requires `rust-analyzer` on PATH (skips cleanly if absent).

Not added: sibling live tool-level tests for `replace`/`remove`/`rename`. Justification — all four `do_*` methods call the exact same `resolve_write_path_for` with the identical argument pattern (verified by direct code reading, not inference), so the fast resolver-level test plus the one live `insert` tool-level test already exercise the shared defective line under both the unfixed and fixed code paths; three more live-LSP round trips would be redundant coverage of the same single line, not new coverage of new logic.

## Workarounds
None were needed once the fix landed. Prior to this fix: callers editing a
specific worktree/foreign workspace with `edit_code` should verify with
`git status` (or `git diff`) in BOTH the pinned workspace and the
session-default project immediately after the call, and revert/re-apply
manually if the edit landed in the wrong one — exactly the self-caught
pattern from the original report. Alternative: switch the session-default
active project to the target workspace (`workspace(activate, path=...)`)
instead of using a `workspace=` pin for `edit_code` calls, since the
session-default path was unaffected by this bug.

## Resume
Fix implemented and verified locally this session (fmt/clippy/test/release
build all green; `edit_code_insert_honors_workspace_override_pin` now
passes). Not yet committed. Remaining: commit the change, then follow the
standard ship sequence (`docs/RELEASE.md`) before cherry-picking to
`master` — including a live `/mcp` reconnect + two-worktree pin verification
matching the regression test's setup, since this session's verification was
library-level (`cargo test`) plus a release build, not a live MCP round
trip. Once cherry-picked, cite the master-side SHA here per CLAUDE.md's
"after cherry-pick" rule, then archive to `docs/issues/archive/`.

## References
- Sibling bug (read-path + LSP-root pin, same pattern, already fixed):
  catalog id `3fb29bc678a32562`,
  `docs/issues/2026-06-11-lsp-tools-ignore-workspace-pin-path.md`, shipped
  `85dc92f9`.
- `docs/plans/2026-05-30-per-request-workspace-pinning.md` — the original
  per-request pinning design (`ToolContext.workspace_override`).
- Original report: external SDD-execution review comment (backend-kotlin,
  `~/work/mirela/backend-kotlin`), 2026-07-09 — "edit_code with a workspace
  pin mis-routed structural inserts to the main repo instead of the worktree
  on all four implementer dispatches... each self-caught via git status and
  reverted cleanly."

