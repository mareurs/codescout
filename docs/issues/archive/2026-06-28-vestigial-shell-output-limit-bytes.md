---
status: fixed
opened: 2026-06-28
closed: 2026-06-28
severity: low
owner: marius
related: []
tags: [config, security, dead-code]
kind: bug
---

# BUG: `shell_output_limit_bytes` config is a silent no-op

## Summary
The `[security] shell_output_limit_bytes` setting was accepted, documented, and
plumbed all the way into `PathSecurityConfig`, but **nothing ever read it**. A user
could set it expecting shell output to be capped at N bytes and get no effect.
Shell output is actually bounded by the `@cmd_*` output-buffer / progressive-disclosure
layer, which made the byte-limit redundant. Removed.

## Symptom (Effect)
Setting `shell_output_limit_bytes` in `.codescout/project.toml` changed nothing.
The only enforcement function was dead:

```rust
#[allow(dead_code)] // Kept as safety net for byte-level shell_output_limit_bytes config.
pub(crate) fn truncate_output(output: &str, limit: usize) -> (String, bool) { … }
```

`references(truncate_output)` returned only the definition — zero callers.

## Reproduction
1. `git rev-parse HEAD` → `57ea015d` (branch `experiments`, pre-fix).
2. Add `shell_output_limit_bytes = 100` under `[security]` in a project's
   `.codescout/project.toml`.
3. Run a `run_command` that emits more than 100 bytes.
4. Observe: output is not truncated at 100 bytes (the buffer layer handles
   sizing instead; the config value is ignored entirely).

## Environment
codescout `experiments` branch, all platforms — config-layer logic, not
platform-specific.

## Root cause
`SecuritySection.shell_output_limit_bytes` → `PathSecurityConfig.shell_output_limit_bytes`
was a write-only field: set by `to_path_security_config()` (`src/config/project.rs`)
but never read by any gate or by `run_command`. Its sole consumer,
`truncate_output()` in `src/tools/run_command/inner.rs`, was annotated
`#[allow(dead_code)]` and never called. Output bounding moved to the output-buffer
layer at some earlier point, orphaning the field.

## Evidence
`grep "\.shell_output_limit_bytes" src/` matched only the plumbing assignment in
`to_path_security_config`; no read site. `references(symbol="truncate_output")`
returned the definition only.

## Hypotheses tried
1. **Hypothesis:** the field is enforced somewhere outside `path_security`.
   **Test:** `grep`/`references` across `src/` for `shell_output_limit_bytes` and
   `truncate_output`. **Verdict:** rejected — no consumer; enforcer is dead code.

## Fix
Removed the field from `SecuritySection` (`src/config/project.rs`),
`PathSecurityConfig` (`src/util/path_security.rs`), and `GlobalSecuritySection`
(`src/config/global.rs`); deleted the dead `truncate_output()`
(`src/tools/run_command/inner.rs`) and the `default_shell_output_limit()` helper.
Done together with the removal of the redundant `shell_enabled` master switch (see
Related). Implemented on `experiments`; master SHA TBD after cherry-pick.

## Tests added
No positive regression test is possible for a removed no-op field. Coverage is via
the drift guard (`grep -rn "shell_output_limit_bytes" src/ tests/` returns nothing)
plus `cargo clippy -- -D warnings` (would flag a re-orphaned helper). The sibling
shell-off control gained `shell_command_mode_disabled_blocks_run_command` in
`src/tools/run_command/tests.rs`.

## Workarounds
N/A — output is already bounded by the `@cmd_*` buffer layer; no user action needed.

## Resume
N/A — fixed. Archive into `docs/issues/archive/` once the fix lands on `master`
(`git branch --contains <fix-sha>` shows `master`).

## References
- Related cleanup: `shell_enabled` master switch removed in the same change
  (redundant with `shell_command_mode = "disabled"`). Migration for anyone who
  had `shell_enabled = false`: use `shell_command_mode = "disabled"` instead —
  the removed field is silently ignored by serde (no `deny_unknown_fields`).
- Plan: `/home/marius/.claude-kat/plans/bright-mixing-falcon.md`
- `src/tools/run_command/inner.rs` (Step 3 — the surviving shell-off gate)
