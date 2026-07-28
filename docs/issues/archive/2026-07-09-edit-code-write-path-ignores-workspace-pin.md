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


## Live-verification finding (post-fix) — FIXED

While doing a live `/mcp`-reconnected verification through the actual running
server (not `cargo test`), attempting a pinned **write** (`edit_code`,
also independently reproduced with `create_file`) to a workspace that had
**never been explicitly `activate`d** consistently failed with:

```
File writes are disabled for this project. If this project was activated
in read-only mode, call workspace(action='activate', read_only: false) to
enable writes.
```

**Root cause of this behavior (not a new bug — pre-existing, by design):**
every internal caller of `Agent::ensure_resident` (`with_project_at`,
`with_project_at_mut`, `call_edges_project_id_for`, `tools/memory/mod.rs`)
passes `read_only: None` unconditionally (`src/agent/mod.rs:591,723,1226`;
`src/tools/memory/mod.rs:479`). `ensure_resident`'s own doc comment states
"Pinned, non-home workspaces default to read-only" — confirmed intentional.
`project_security_config` (`agent/mod.rs:361-373`) then forces
`file_write_enabled = false` whenever `p.read_only` is true, regardless of
that workspace's own `project.toml` setting. Separately, `Agent::activate`
unconditionally does `inner.workspaces.clear()` (`agent/mod.rs:518`) on
**every** call — home or foreign — so there is no way to have two
simultaneously-resident, simultaneously-writable workspaces via the tools
exposed today. A workspace is writable if and only if it is the **current**
session default (most recently `activate`d, nothing else activated since).

**Practical consequence of combining this session's two fixes
(write-path + `check_tool_access`):** before this session, a pinned write to
a workspace that was never explicitly activated would **silently succeed but
land in the wrong (session-default) file** — the exact bug this file
documents. After this session's fixes, the identical calling pattern (pin
only, no `activate`) now **fails loudly and safely** with "file writes
disabled" instead — an improvement (fail-closed beats silent misdirection),
but it means the write no longer *works* for that pattern at all, only fails
safely. For the original bug's fix to actually deliver a **successful,
correctly-routed write** in the field (not just a safe rejection), the caller
must ensure the pin target is already resident+writable — today that means
explicitly calling `workspace(action='activate', path=<target>,
read_only=false)` on it at some point with nothing else activated since,
which the workspace-state guide otherwise discourages from subagents
precisely because it clobbers every other resident workspace
(`docs/plans/2026-05-30-per-request-workspace-pinning.md` design docs confirm
this read-only default and single-slot-clear were deliberate Phase 1
choices, not oversights — but the *interaction* with a now-correctly-pinned
`check_tool_access` doesn't appear to have been previously exercised or
documented).

This is almost certainly why the original report's four dispatches saw
writes *succeed* (misrouted) rather than rejected: `check_tool_access` was
still using the session-default's (permissive, presumably explicitly
activated) config at that time, masking this read-only-default entirely.
Fixing `check_tool_access` to honor the pin (this session) makes the access
gate correctly consult the pin target's config — which, for a never-activated
target, is always read-only.

**Not filed as a separate bug** — recorded here since it's a direct,
material consequence of this fix's interaction with pre-existing,
deliberate design. Worth a design discussion: should `ensure_resident`
(or a new pin-time parameter) support opting a per-request pin into write
access without requiring a full `activate` that clobbers sibling residents?
That would need its own plan/spec, not a bug fix.

**Fixed this session, on top of the original fix above.** `Agent::ensure_resident` (`src/agent/mod.rs`) now upgrades an already-resident, read-only entry to writable when called with `Some(false)`, instead of no-op'ing on the idempotence check. `CodeScoutServer::call_tool_inner` (`src/server.rs`) now calls `ensure_resident(root, Some(false))` for any write-tool call (`tool.is_write(&input)`) that carries a `workspace=` pin, *before* `check_tool_access` runs — so a pin to a workspace that was never separately `activate`d now succeeds at writing (the pin itself is treated as the caller's explicit consent), instead of failing "file writes disabled". Read-only pinned calls are unaffected — they never reach this branch, so a pinned read still gets the safer read-only default. Path-security scoping (`validate_write_path`, the outside-root ack flow, deny-lists) is untouched; this only affects the `read_only`/`file_write_enabled` gate.

Tests added: `Agent::ensure_resident_upgrades_read_only_pin_to_writable` (`src/agent/mod.rs`) at the accessor level; `CodeScoutServer::call_tool_inner_grants_write_access_to_a_fresh_pinned_workspace` (`src/server.rs`) end-to-end through the real dispatch path, pinning `create_file` to a workspace that is never `activate`d and asserting the write succeeds.
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

Live `/mcp`-reconnected verification done. Read-side pin behavior confirmed
correct through the actual running server (`symbols(path, workspace=...)`
resolves against the pinned workspace). Write-side verification surfaced the
read-only-by-default finding documented above rather than a clean
file-identity demo — that finding is more important than the demo would have
been. Next: decide whether to open a design discussion/plan for a supported
way to grant a per-request pin write access without a full `activate`
(see the finding above); until then, document in onboarding/CLAUDE.md-level
guidance that pinned writes to a never-activated workspace will be safely
rejected, not silently misrouted. Commit `3fca32db` on `experiments` also
fixed 3 sibling bugs found in a follow-up audit
(`docs/issues/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md`).
Not yet cherry-picked to `master`.
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
