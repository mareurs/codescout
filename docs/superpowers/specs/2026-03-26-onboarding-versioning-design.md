# Onboarding Versioning

**Date:** 2026-03-26
**Status:** Draft
**Component:** `src/tools/workflow.rs`, `src/config/project.rs`, `CLAUDE.md`

## Problem

When codescout's tool API changes (renames, new parameters, consolidated tools),
the system prompt (`.codescout/system-prompt.md`) becomes dangerously stale — it
contains guidance referencing wrong tool names and signatures. Every session using
the stale prompt gets incorrect tool routing advice.

There is no mechanism to detect this staleness or trigger a refresh. Projects
onboarded with an older codescout version silently use outdated guidance until
someone manually re-runs onboarding with `force=true`.

## Solution

A **compiled version constant** (`ONBOARDING_VERSION`) in the codescout binary,
compared against a **stored version** in `project.toml`. When the stored version
is missing or lower than the compiled version, `onboarding()` automatically
dispatches a lightweight subagent to refresh the system prompt from existing
memories — no full re-exploration needed.

This is agent-agnostic: the version check lives server-side in the `onboarding()`
tool, not in any client plugin.

## Design

### Version Constant and Storage

**Compiled constant** in `src/tools/workflow.rs`:

```rust
/// Bump this when system prompt surfaces change significantly.
/// Missing or lower stored version triggers auto-refresh of the system prompt.
const ONBOARDING_VERSION: u32 = 1;
```

**Stored version** in `.codescout/project.toml`, `[project]` section:

```toml
[project]
name = "my-project"
languages = ["rust"]
encoding = "utf-8"
onboarding_version = 1
```

**Config struct** in `src/config/project.rs`:

```rust
pub struct ProjectSection {
    pub name: String,
    pub languages: Vec<String>,
    pub encoding: String,
    // ... existing fields ...
    #[serde(default)]
    pub onboarding_version: Option<u32>,
}
```

`Option<u32>` with `serde(default)` means existing `project.toml` files without
the field deserialize as `None` — treated as stale (no version = needs refresh).

### Onboarding Dispatch Matrix

| Call | Condition | Action | Subagent? |
|------|-----------|--------|-----------|
| `onboarding()` | Not onboarded | Full exploration + memories + system prompt | Yes (Sonnet, heavy) |
| `onboarding()` | Onboarded, version OK | Return status | No |
| `onboarding()` | Onboarded, version stale | Refresh system prompt from existing memories | Yes (Sonnet, lightweight) |
| `onboarding(force=true)` | Any | Full exploration + memories + system prompt | Yes (Sonnet, heavy) |
| `onboarding(refresh_prompt=true)` | Any (onboarded) | Refresh system prompt from existing memories | Yes (Sonnet, lightweight) |

`force=true` takes priority over `refresh_prompt=true` if both are set.

### Fast Path Logic Change

Current:

```
if has_config && has_onboarding_memory → return "already onboarded"
```

New:

```
if has_config && has_onboarding_memory:
    if stored_version is None or stored_version < ONBOARDING_VERSION:
        → build lightweight subagent prompt for system prompt refresh
        → return two-block response (same shape as full onboarding)
    else:
        → return "already onboarded" (unchanged)
```

### `refresh_prompt` Parameter

Added to `input_schema`:

```json
{
  "type": "object",
  "properties": {
    "force": {
      "type": "boolean",
      "description": "Force full re-scan even if already onboarded (default: false)"
    },
    "refresh_prompt": {
      "type": "boolean",
      "description": "Regenerate system prompt from current templates without re-exploring (default: false)"
    }
  }
}
```

### System Prompt Refresh — Why It Needs a Subagent

The system prompt (`.codescout/system-prompt.md`) is **LLM-synthesized**, not
template-generated. During onboarding, the LLM reads the codebase, understands the
project, and writes a concise prompt with project-specific entry points, key
abstractions, search tips, and navigation strategy.

`build_system_prompt_draft()` produces a scaffold/template, but the actual system
prompt is a synthesis of that template + project knowledge from memories. Refreshing
it requires an LLM to:

1. Read existing memories (architecture, conventions, etc.)
2. Read the current system prompt
3. Regenerate using current tool guidance and the existing project knowledge
4. Write the updated file

This is much lighter than full onboarding (reads ~6 memories vs exploring the
whole codebase), but still requires a subagent call.

### Lightweight Subagent Prompt

New builder function: `build_prompt_refresh_subagent_prompt(memory_topics: &[String])`.

The subagent prompt instructs:

```
You are a system prompt refresh subagent for codescout. The project's tool API has
been updated and the system prompt needs regenerating with current tool guidance.

FIRST ACTION: Call activate_project(".", read_only: false)

Then:
1. Read these existing memories to understand the project:
   [list of memory topics from the project]
2. Read the current system prompt: read_markdown(".codescout/system-prompt.md")
3. Regenerate the system prompt following this template:
   [system prompt template from onboarding_prompt.md section 7]
4. Write the updated system prompt: create_file(".codescout/system-prompt.md", ...)

Key constraint: Do NOT re-explore the codebase. Use the existing memories as your
source of project knowledge. The goal is to update tool guidance and formatting,
not to rediscover the project.

LAST ACTION: Call activate_project(".") to restore project state.

Return a brief summary of what changed in the system prompt.
```

The response uses the same two-block shape as full onboarding:
- Block 1: `main_agent_instructions` (dispatch command)
- Block 2: delimited `subagent_prompt`

The `main_agent_instructions` for a version-triggered refresh includes context:
"System prompt is outdated (v{old} → v{new}). Dispatching a lightweight subagent
to refresh it from existing memories."

### Version Write

The stored `onboarding_version` in `project.toml` is updated in two places
(belt-and-suspenders):

1. **Server-side optimistic write:** `Onboarding::call` writes the version to
   `project.toml` immediately before returning the refresh/onboarding response.
   This ensures the version is always updated, even if the subagent fails or
   forgets. If the subagent fails, the version is "wrong" (claims current when
   the prompt is stale), but the user will notice and can re-run. This is strictly
   better than the inverse (subagent succeeds but version never written, causing
   infinite re-triggers on every session).

2. **Subagent epilogue write:** The subagent epilogue also includes a version
   write instruction as a redundancy measure. In practice this is a no-op (same
   value already stored), but it makes the subagent self-contained.

Log the version transition: `tracing::info!("onboarding version stale: stored={:?}
current={}", stored, ONBOARDING_VERSION)` when a refresh triggers.
### Response Shape for Version-Triggered Refresh

When the version is stale and auto-refresh triggers, the response looks like:

```json
{
  "onboarded": true,
  "version_stale": true,
  "stored_version": 1,
  "current_version": 2,
  "languages": ["rust"],
  "config_created": false,
  "subagent_prompt": "<lightweight refresh prompt>",
  "main_agent_instructions": "System prompt outdated (v1 → v2). Spawn a general-purpose subagent with model=sonnet..."
}
```

For `refresh_prompt=true`, same shape but `version_stale` may be false (explicit
refresh regardless of version).

### `call_content` Routing Discriminator

`call_content` must distinguish three response types:

1. **Fast path** (version OK, no subagent) — `onboarded: true`, no `subagent_prompt`
2. **Full onboarding** (not onboarded) — has `subagent_prompt`, no `onboarded: true`
3. **Version refresh** (stale version) — `onboarded: true` AND has `subagent_prompt`

The discriminator is **presence of `subagent_prompt` field:**

```rust
if val.get("subagent_prompt").is_some() {
    // Two-block response (full onboarding OR version refresh)
} else if val["onboarded"].as_bool().unwrap_or(false) {
    // Single-block fast path
}
```

This works for all three subagent paths (full onboarding, auto version refresh,
explicit `refresh_prompt=true`) — they all set `subagent_prompt`. Only the fast
path omits it.

### Response for Successful Fast Path (Version OK)

Unchanged from current behavior — `{"onboarded": true, "message": "..."}`. No
new fields needed when version is current.

## Rust Code Changes

| Component | File | Change | Size |
|---|---|---|---|
| Version constant | `src/tools/workflow.rs` | `const ONBOARDING_VERSION: u32 = 1;` | 3 lines |
| Config field | `src/config/project.rs` | Add `onboarding_version: Option<u32>` to `ProjectSection` | 3 lines |
| Fast path version check | `src/tools/workflow.rs` | Read stored version, compare, branch to refresh | ~20 lines |
| Optimistic version write | `src/tools/workflow.rs` | Write version to project.toml before returning refresh response | ~10 lines |
| `build_prompt_refresh_subagent_prompt()` | `src/tools/workflow.rs` | New function building lightweight subagent prompt | ~40 lines |
| `build_prompt_refresh_main_instructions()` | `src/tools/workflow.rs` | Dispatch instructions for refresh path | ~15 lines |
| `refresh_prompt` parameter | `src/tools/workflow.rs` | Add to `input_schema`, parse in `call` | ~10 lines |
| Version write in full onboarding | `src/tools/workflow.rs` | Write version after building subagent_prompt (optimistic) + epilogue instruction (redundancy) | ~5 lines |
| `call_content` discriminator | `src/tools/workflow.rs` | Use `val.get("subagent_prompt").is_some()` instead of `!val["onboarded"]` to route to two-block path | ~5 lines |
| CLAUDE.md rule | `CLAUDE.md` | Add version bumping rule to Prompt Surface Consistency section | ~15 lines |

**Total:** ~130 lines new/modified across 3 files.
## Edge Cases

### Pre-versioning projects (no stored version)

`onboarding_version: None` in deserialized config. Treated as stale — triggers
system prompt refresh on next `onboarding()` call. After refresh, version is
written, and subsequent calls hit the fast path.

### Project has config but no system prompt file

The refresh subagent will create `.codescout/system-prompt.md` from scratch using
memories. This is fine — the prompt template handles both create and update.

### Memories are stale or missing

The refresh subagent reads whatever memories exist. If memories are missing (e.g.,
`architecture` was deleted), the regenerated system prompt will be less detailed
but still have correct tool guidance. The user can run `force=true` to do a full
re-exploration if they want better memories.

### `refresh_prompt=true` on a project that's never been onboarded

Return a `RecoverableError`: "Project not yet onboarded — no memories to build a
system prompt from. Run onboarding() first to explore the codebase and create
memories."

The check uses the same gate as the fast path: `has_config && has_onboarding_memory`.
If a project has config + memories written manually, `refresh_prompt=true` is allowed.

### Programmatic memories not refreshed

The lightweight refresh only regenerates `.codescout/system-prompt.md`. Programmatic
memories like `language-patterns` (written by `build_language_patterns_memory()`)
and `onboarding` (the summary) are **not** updated during a version refresh. They
describe the project, not codescout's API. Use `force=true` for a full re-onboarding
if these need updating.
### Version goes backward (downgrade)

If `stored_version > ONBOARDING_VERSION` (user downgrades codescout), treat as
current — do not refresh. Only refresh when stored < compiled. This prevents
unnecessary churn on downgrades.

Log a warning: `tracing::warn!("stored onboarding version ({stored}) is newer than
compiled ({compiled}) — skipping refresh. Run onboarding(force=true) if you
downgraded intentionally.")`
## CLAUDE.md Addition

Add to the "Prompt Surface Consistency" section:

```markdown
### Onboarding Version

When modifying system prompt surfaces, bump `ONBOARDING_VERSION` in
`src/tools/workflow.rs`. This triggers automatic system prompt refresh for all
projects onboarded with the previous version.

Bump when the generated system prompt would reference tool names, parameters,
or workflows that no longer exist:
- Tool names change (rename, consolidate)
- Tool parameter semantics change
- Server instructions (`server_instructions.md`) change significantly
- Onboarding prompt templates change in ways that affect the generated system prompt

Do NOT bump for:
- Bug fixes that don't change tool behavior
- Internal refactors
- Memory template changes (memories are re-read during refresh anyway)
```
## Testing

- Unit test: `ProjectSection` deserializes `onboarding_version` correctly (present, absent, null)
- Unit test: version check logic — `None < ONBOARDING_VERSION`, `0 < 1`, `1 == 1`
- Unit test: `onboarding()` returns refresh response when version is stale
- Unit test: `onboarding()` returns normal fast path when version is current
- Unit test: `onboarding(refresh_prompt=true)` returns refresh response even when version is current
- Unit test: `onboarding(refresh_prompt=true)` errors when not onboarded
- Unit test: `force=true` takes priority over `refresh_prompt=true`
- Unit test: `build_prompt_refresh_subagent_prompt()` contains memory read instructions and template
- Unit test: `call_content` returns two blocks for refresh path
- Manual E2E: build release, test on project with no version → triggers refresh
