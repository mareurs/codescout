---
id: '7271b9b25e4e5b42'
kind: bug
status: fixed
title: 'BUG: residual workspace_override pin gaps found while auditing the edit_code write-path bug (peer.rs, server.rs::post_process, usage telemetry x2, lsp_mux_override, onboarding.rs)'
tags:
- workspace-pin
- audit
- residual
---

## Summary
After fixing the `edit_code` write-path pin bug (`docs/issues/2026-07-09-edit-code-write-path-ignores-workspace-pin.md`), a 5-way parallel audit swept the rest of the codebase for the same "plain vs `_for`-pinned Agent method" pattern. Three CONFIRMED bugs from that audit were fixed in the same session (`symbols.rs` glob-path resolution, `auto_register.rs`, and `server.rs::call_tool_inner`'s `check_tool_access`/`timeout_secs`). This file tracks the **6 remaining LIKELY GAP findings that were deliberately deferred** — lower severity than the 3 fixed bugs (telemetry attribution, a tool that was never wired for pinning at all, etc.), not partial-migration regressions with the same "sibling call in the same function is pinned but this one isn't" smoking gun.

## Findings

### 1. `src/tools/peer.rs:47` — `PeerTool` advertises a pin it doesn't honor
`PeerTool` takes `ctx: &ToolContext` and never overrides `Tool::pinnable()` (defaults to `true`), so its schema gets a `workspace` param injected and `ctx.workspace_override` is populated whenever a caller passes `workspace=...`. But the body resolves the peer-registry path (`.codescout/peers.toml`) via the plain, unpinned `ctx.agent.project_root()`. No sibling `_for` call exists in the function, so this doesn't meet the strict "same-function inconsistency" bar, but since the tool's own schema actively claims pin support, treat as high-confidence.
**Fix idea:** `ctx.agent.project_root_for(ctx.workspace_override.as_deref())`.

### 2. `src/server.rs::post_process` — never receives the override at all
Called at the end of `call_tool_inner` with only `tool_name: &str` — no `ctx`/override threaded through. Resolves `self.agent.project_root()` (plain) to strip the project-root prefix and emit the "paths are relative to `<root>`" annotation. Under a pin, output gets stripped/annotated against the wrong root.
**Fix idea:** add a `workspace_override: Option<&Path>` param to `post_process`, pass `ctx.workspace_override.as_deref()` from its one call site in `call_tool_inner`, and swap to `project_root_for`.

### 3. `src/tools/usage.rs:38-40` — `GetUsageStats::call` ignores the pin
Found independently by 3 of the 5 audit agents (strongest corroboration in the whole audit). Resolves `project_root` via plain `with_project` before opening the project-scoped usage DB. Telemetry-only — wrong-project stats, no data corruption.
**Fix idea:** `ctx.agent.with_project_at(ctx.workspace_override.as_deref(), |p| Ok(p.root.clone()))`.

### 4. `src/usage/mod.rs:53` — `UsageRecorder::write_content` ignores the pin
`UsageRecorder::write_content` resolves the usage-db project root via plain `with_project`. Its caller, `call_tool_inner`, already computes `ctx.workspace_override` in the same function and (after this session's fix) correctly threads it into `check_tool_access`/`timeout_secs`/the write-guard — but `UsageRecorder::new(self.agent.clone(), ...)` is constructed without the override, so every pinned call's usage.db attribution silently lands in the session-default project's `usage.db`, not the pinned one.
**Fix idea:** thread `ctx.workspace_override` (or the resolved root) into `UsageRecorder::new`/`write_content`.

### 5. `Agent::lsp_mux_override` (`agent/mod.rs:1401`) — plain `with_project`
Called from pin-aware, `ctx`-bearing call sites in `symbols.rs` (search_files_restricted and others), `list_overview.rs`, and `symbol/query.rs` — same LSP-config-resolution class as the already-fixed read-path bug (commit `85dc92f9`), but this specific helper was missed by that fix.
**Fix idea:** add a `workspace_override` param, thread `ctx.workspace_override.as_deref()` from all call sites.

### 6. `src/tools/onboarding.rs` — never wired for per-request pinning at all
Confirmed by 2 of the 5 audit agents: **all 11** production call sites (`Onboarding::call`, `call_content`, `handle_refresh_prompt`, `handle_already_onboarded`, `probe_index_status`, `write_onboarding_memories`, `gather_per_project_protected`, `perform_full_onboarding`) use the plain `require_project_root`/`with_project`/`reload_config_if_project_toml`, with zero `_for`/`_at` calls anywhere in the file. This is qualitatively different from the other findings — not a partial migration that regressed, but a tool that was never wired for pinning in the first place. Onboarding writes `.codescout/project.toml` and onboarding memory files, so a `workspace=` pin on it would silently onboard/write into the session-default project instead of the intended one.
**Fix idea:** thread `ctx.workspace_override` through all 11 sites, swapping to `require_project_root_for`/`with_project_at`/`reload_config_if_project_toml_for`. Larger, more mechanical fix than the others — good candidate for its own dedicated session given the site count.

## Root cause (shared across all 6)
Same shape as the `edit_code` bug and its predecessor (`3fb29bc678a32562`): `ctx.workspace_override` is populated generically for every tool call in `server.rs::call_tool_inner` regardless of a tool's own schema, but individual call sites across the codebase were migrated to the pin-aware `_for`/`_at` accessors incrementally and unevenly. These 6 are the sites two prior migration passes (the `85dc92f9` read-path/LSP-root fix and this session's write-path fix) didn't reach.

## Severity
Low-to-medium across the board — no data-corruption risk (unlike the 3 fixed bugs, which could silently write/mutate the wrong project). Worst case is wrong-project telemetry (#3, #4), a stale/wrong root name in a `.codescout/peers.toml` (#1), a misleading path-relative banner (#2), a wrong LSP mux config (#5), or onboarding writing into the wrong project (#6).

## Status

**fixed (2026-07-13)** — all 6 findings migrated to the pin-aware accessors, each with its own regression test. Branch `experiments`.
## Tests added

One per finding, all following the established two-workspace pattern (default = B, pin THIS call to A, assert the effect landed in A and NOT in B):

| # | Test | Location |
|---|---|---|
| 1 | `peer_tool_honors_workspace_override_pin` | `src/tools/peer.rs` |
| 2 | `post_process_strips_and_annotates_against_the_pinned_root` | `src/server.rs` |
| 3 | `get_usage_stats_honors_workspace_override_pin` | `src/tools/usage.rs` |
| 4 | `record_content_honors_workspace_override_pin` | `src/usage/mod.rs` |
| 5 | `lsp_mux_override_resolves_pin_over_default` | `src/agent/mod.rs` |
| 6 | `onboarding_honors_workspace_override_pin` | `src/tools/run_command/tests.rs` |

Every test was confirmed to FAIL before its fix and pass after. For findings 2 and 4 — where the signature change and the fix landed in the same edit, so no natural RED was observed — the fix was mutated back to the unpinned accessor to prove the test actually catches the bug, then restored. Finding 2's mutation check is the most instructive: it caught workspace A's ABSOLUTE path (`/tmp/.tmpXXXX/src/lib.rs`) leaking unstripped into the response, confirming the cross-workspace path leak was real and not theoretical.

Full suite after: 3196 passed, 0 failed. `cargo fmt` + `cargo clippy --all-targets -- -D warnings` clean.
## Resume

N/A — fixed. Cherry-pick to `master` still pending; cite the master-side SHA here after the pick, per CLAUDE.md § "After cherry-pick", before archiving to `docs/issues/archive/`.
## References
- `docs/issues/2026-07-09-edit-code-write-path-ignores-workspace-pin.md` — the 3 CONFIRMED bugs fixed this session (symbols.rs glob path, auto_register.rs, server.rs::call_tool_inner).
- `docs/issues/2026-06-11-lsp-tools-ignore-workspace-pin-path.md` (catalog id `3fb29bc678a32562`) — the earlier read-path/LSP-root fix (`85dc92f9`) that these 6 findings fell outside the scope of.
