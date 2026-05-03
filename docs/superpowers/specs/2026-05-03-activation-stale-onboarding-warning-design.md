# Activation Stale Onboarding Warning

**Date:** 2026-05-03  
**Status:** Approved

## Problem

`onboarding_version_stale` is only checked when the LLM explicitly calls `onboarding()`.
If the LLM skips that call — possible when the system prompt is itself stale and doesn't
guide it to run onboarding — the version mismatch is invisible for the entire session.
Tool names and signatures may have changed, but the LLM operates on an outdated mental model.

## Goal

Surface the stale-onboarding signal at `activate_project` time, before any tool calls.
One-shot warning: prominent enough to act on, silent on the happy path.

## Design

### Detection

`build_activation_response` (`src/tools/config/mod.rs`) already reads project config via
`with_project`. Add `onboarding_version: Option<u32>` to the read tuple, then call
`onboarding_version_stale(stored)` (already `pub(crate)` in `src/tools/onboarding.rs`)
after the block. No new structs, no new files.

### JSON shape

When stale, inject a top-level `system_prompt_stale` object:

```json
"system_prompt_stale": {
  "stored_version": 20,
  "current_version": 22,
  "action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
}
```

When not stale: field absent. Zero noise on the happy path.

### Compact format

`format_activate_project` (`src/tools/config/mod.rs`) prepends a warning line when
`system_prompt_stale` is present in the JSON:

```
⚠ SYSTEM PROMPT STALE (v20 → v22): run onboarding(action="refresh_prompt") now.
[...normal compact output...]
```

The compact format is what the LLM sees in exploring mode, so the warning appears
regardless of whether the LLM reads the raw JSON.

## Files Changed

- `src/tools/config/mod.rs` — `build_activation_response` + `format_activate_project`

No prompt surface changes. No `ONBOARDING_VERSION` bump (this change does not alter
tool names or signatures — it only surfaces an existing staleness signal earlier).

## Tests

All in `src/tools/config/` alongside existing activation tests:

| Test | Asserts |
|------|---------|
| `activation_response_includes_stale_warning_when_version_behind` | `system_prompt_stale` present, correct `stored_version` / `current_version` / `action` |
| `activation_response_no_stale_warning_when_version_current` | no `system_prompt_stale` field |
| `format_activate_project_prepends_warning_when_stale` | compact output starts with `⚠ SYSTEM PROMPT STALE` |
| `format_activate_project_no_warning_when_current` | no warning line |

No new test infra required — existing activation test helpers apply.

## Out of Scope

- Repeating the warning on every tool call (one-shot at activation is sufficient)
- Auto-triggering the refresh at activation (user confirmed warn-only)
- Bumping `ONBOARDING_VERSION` (no tool surface change)
