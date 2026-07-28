---
status: fixed
opened: 2026-06-27
closed: 2026-06-27
severity: medium
owner: marius
related: ["docs/trackers/structural-edit-gate-session-log.md", "docs/issues/2026-06-13-rust-lsp-mux-spawn-fail-deadlocks-source-editing.md"]
tags: ["edit_file", "structural-edit", "agentic-surface", "error-message"]
kind: bug
---

# BUG: edit_file structural-gate generic message preempted the richer guidance, nudging a native-Edit escape

## Summary
On a source file with `debug_enforce_symbol_tools = true`, a batch `edit_file`
containing one structural edit was rejected with a *generic* message
("edit_file is blocked for structural edits…") that preempted the richer
always-on guidance — which would have said *which* batch indices were safe and
that single-line literal edits are allowed. The model over-generalized
"structural edits are blocked" to a one-token modifier change
(`class X` → `data class X`), concluded `edit_code` had no add-modifier
operation, and escaped to the native `Edit` tool. Affects any agent editing
source under the flag (set `true` in codescout's own `.codescout/project.toml`;
observed live in `backend-kotlin`).

## Symptom (Effect)
A batch `edit_file` (2 KDoc edits + 1 `class` → `data class`) returned:

```
edit_file is blocked for structural edits on source code files (debug_enforce_symbol_tools is enabled)
```

with hint "Use edit_code(...) for structural changes." No indication that the
two KDoc edits were safe, nor that a single-line modifier edit would pass. The
session fell back to the native `Edit` tool for the one-token change.

## Reproduction
git HEAD at observation: `4bdf0a2d389776d1361c9401cc7ea4cce843aacd`. With
`.codescout/project.toml` `[security] debug_enforce_symbol_tools = true`, call
`edit_file` on a `.kt`/`.rs` source with a batch mixing a comment edit and a
multi-line `class X` → `data class X` edit. The `debug_enforce_symbol_tools`
early gate returns the generic message, preempting the batch pre-pass's rich
"indices [0,1] safe, split like this" message.

## Environment
codescout MCP, `experiments` branch, any project with
`debug_enforce_symbol_tools = true`. Observed in
`/home/marius/work/mirela/backend-kotlin`.

## Root cause
Two overlapping enforcement layers in `src/tools/edit_file/mod.rs`: the
config-gated `debug_enforce_symbol_tools` early gate (was at `mod.rs:362`) and
the always-on `guard_structural_rewrite` (batch `mod.rs:429`, single
`mod.rs:583`). Both call the same predicate (`is_structural_edit` →
`guard_structural_rewrite`) and block an identical set — but the flag gate
returned earlier with a *generic* message, preempting the always-on path's
richer per-edit guidance. Introduced by commit `fbd8bbdc` (narrowed the flag to
structural-only, reusing the guard predicate, leaving two parallel enforcement
points diverging only in message). Compounding it: neither always-on message
mentioned that single-line literal edits (including modifier tweaks) are
allowed.

## Evidence
- F-1 in `docs/trackers/structural-edit-gate-session-log.md` (full scout).
- Identical blocked-set regardless of flag, proven by the test pair:
  `batch_edit_blocks_new_symbol_introduction_via_new_string` (default config,
  blocks) vs the former `edit_file_blocked_on_source_file_when_debug_enforce_symbol_tools`
  (flag on, same block).
- Live repro: a single-line `class X` → `data class X` `edit_file` on a scratch
  `.kt` returned `"ok"` even with the flag on — the gate is diff-aware and never
  checks keywords on a single line.

## Hypotheses tried
1. **Hypothesis:** the gate genuinely blocks a single-line modifier edit.
   **Test:** live single-line `class` → `data class` edit on a scratch `.kt`.
   **Verdict:** rejected — it passed (gate is diff-aware; single-line skips the
   keyword check). **Evidence:** Evidence §3.
2. **Hypothesis:** the flag adds blocking power over the always-on guard.
   **Test:** compared flag-on vs default-config gate tests.
   **Verdict:** rejected — identical blocked-set; the flag changed only the
   message. **Evidence:** Evidence §2.

## Fix
Two commits on `experiments` (master-side SHAs to be filled after cherry-pick):
- **Commit 1** — retire `debug_enforce_symbol_tools`: delete the early gate and
  the now-dead `is_structural_edit`; remove the field from `SecuritySection`
  (`src/config/project.rs`) and `PathSecurityConfig` (`src/util/path_security.rs`)
  plus the mapping and the `.codescout/project.toml` line; delete the two
  redundant flag tests; drop the defunct message line in
  `src/prompts/guides/iron-laws-detail.md`. The always-on
  `guard_structural_rewrite` becomes the sole enforcement, emitting the richer
  message.
- **Commit 2** — extend the single-edit rejection hint in
  `guard_structural_rewrite` (`src/tools/edit_file/mod.rs`) to point at the
  single-line escape hatch; tighten `iron-laws-detail.md` "Gate fires when" to
  state multi-line-changed-region + "single-line literal edits are always
  allowed".

## Tests added
- `config::project::tests::stale_debug_enforce_symbol_tools_key_is_ignored_not_rejected`
  (`src/config/project.rs`) — stale-key tolerance after field removal.
- `structural_rejection_hint_mentions_single_line_escape_hatch`
  (`src/tools/edit_file/tests.rs`) — single-edit rejection hint mentions the
  single-line allowance.

## Workarounds
Make the modifier change as a single-line `edit_file` replacement of just the
token (`class X` → `data class X`); it passes the gate. (Now surfaced directly
in the rejection hint.)

## Resume
N/A — fixed and verified on `experiments` (fmt + clippy `-D warnings` + full lib
green). Cherry-pick to `master`, fill master-side SHAs in §Fix, then archive
after `git branch --contains <sha>` shows `master`.

## References
- `docs/trackers/structural-edit-gate-session-log.md` (F-1, W-1)
- `docs/issues/2026-06-13-rust-lsp-mux-spawn-fail-deadlocks-source-editing.md`
  (the LSP-down deadlock that shares the always-on guard's residual; out of
  scope here)
- Plan: `/home/marius/.claude/plans/scalable-dazzling-scott.md`
