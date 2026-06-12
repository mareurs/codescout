---
id: e492592986c67138
kind: bug
status: fixed
title: 'BUG: onboarding writes the system prompt via memory(topic="system-prompt") → lands in .codescout/memories/, but the always-on injection reads the root .codescout/system-prompt.md — onboarded projects silently get an empty "## Custom Instructions"'
owners:
- marius
tags:
- onboarding
- memory
- prompt-builder
- system-prompt
- server-instructions
topic: null
time_scope: null
---

# BUG: onboarding writes the system prompt via `memory(topic="system-prompt")` → lands in `.codescout/memories/`, but the always-on injection reads the root `.codescout/system-prompt.md`

**Severity:** high · **Opened:** 2026-06-12 · **Status:** open

## Summary
codescout's headline "always-on project guidance" is delivered by reading the **root** `.codescout/system-prompt.md` in `project_status()` and appending it to `server_instructions` as a `## Custom Instructions` section. But **onboarding never writes that file.** Both the single-project and workspace flows instruct the agent to write the system prompt via `memory(action="write", topic="system-prompt", content="...")`, which `MemoryStore::write` routes to `.codescout/**memories**/system-prompt.md` (no special case for this topic). Nothing in codescout bridges the two paths — there is no `fs::write` to the root file anywhere in `src`. Result: a freshly-onboarded project has its system prompt sitting in the on-demand memory store while the always-on injection reads an absent root file and falls through to the (usually empty) deprecated `project.toml` `system_prompt`. The onboarding completion message nonetheless tells the user the system prompt is "always-on … injected into every session" — so the failure is silent and actively mis-reported.

## Symptom (Effect)
After onboarding a project, the always-on `## Custom Instructions` block injected into every MCP session is **empty** (or the deprecated TOML fallback), even though onboarding reported writing the system prompt successfully. The content the agent wrote is reachable only via `memory(action="read", topic="system-prompt")` — i.e. on-demand, the exact opposite of the documented "always-on, injected" contract.

Observed concretely in a consumer repo (claude-plugins, codescout-companion development): the repo had **two** divergent files — `.codescout/system-prompt.md` (root, served to subagents by the companion + read by `project_status`) and `.codescout/memories/system-prompt.md` (where onboarding's write landed; surfaced to the main agent via the companion's memory pointer). They had drifted (root served stale "Code Explorer" + GitHub guidance; the memory had newer content) and had to be hand-synced.

## Reproduction
1. Onboard any project: `onboarding()` and follow the emitted plan.
2. The plan's system-prompt step (single- or workspace-mode) instructs: `memory(action="write", topic="system-prompt", content="...")`.
3. After completion:
   - `ls .codescout/system-prompt.md` → **absent** (or pre-existing/stale), so `project_status().system_prompt` is `None`/stale.
   - `ls .codescout/memories/system-prompt.md` → **present** (this is where the content went).
   - The MCP `initialize.instructions` for a new session has **no** `## Custom Instructions` section (or a stale one), despite onboarding reporting success.

## Environment
- codescout MCP server, master/experiments as of 2026-06-12.
- Discovered while designing a `codescout-companion` change (claude-plugins repo). The companion made the split visible: its SubagentStart hook injects the **root** file verbatim, while its SessionStart hook points the main agent at the **`system-prompt` memory topic** — so each companion path happened to read a different one of the two files.

## Root cause
The onboarding prompt text and the live `memory` tool semantics disagree about where `topic="system-prompt"` lands, and the injection reads a third expectation (the root file).

- **Onboarding instruction (both flows).** `src/prompts/source.md:~325` — "For `system-prompt`, apply the `workspace-scope: system-prompt` section (single-project flow treats the project as its own workspace)." The included template `src/prompts/memory-templates.md:255-275` says the topic is *"rendered into `.codescout/system-prompt.md`"* and is written via `memory(write)`. `src/prompts/workspace_onboarding_prompt.md:211-212` is explicit (and false): "`system-prompt` — write to `.codescout/system-prompt.md` via `memory(action: "write", topic: "system-prompt", content: ...)`." `src/prompts/builders.rs:909` (`build_synthesis_prompt`, Step 3) emits the same `memory(write, topic="system-prompt")` instruction.
- **Where it actually lands.** `memory` write branch (`src/tools/memory/mod.rs`, write arm) → `resolve_memory_dir` (→ `.codescout/memories/`) → `MemoryStore::from_dir(...).write(topic, content)`. `MemoryStore::write` (`src/memory/mod.rs:73-80`) writes `self.topic_path(topic)`; `topic_path` (`src/memory/mod.rs:123-126`) = `memories_dir.join(sanitize_topic(topic)).with_extension("md")`. **No special case for `system-prompt`.** So the write goes to `.codescout/memories/system-prompt.md`, never the root.
- **Where the always-on injection reads.** `project_status()` (`src/agent/mod.rs:1101`) reads `project.root.join(".codescout").join("system-prompt.md")` (the **root**), falling back to `project.config.project.system_prompt` (TOML, deprecated per `src/config/project.rs:36`) at `:1115`. `build_server_instructions()` (`src/prompts/mod.rs`, tail of fn `27-121`) appends `status.system_prompt` as `## Custom Instructions`. `server.rs:103` (`from_parts`) computes this at session/server construction → MCP `initialize.instructions`; `server.rs:478` re-sends on project change.

There is **no rendering/sync step** from the memory file to the root file (grep confirms no `fs::write` to `system-prompt.md` in `src`; `MemoryStore::write` has no topic special-case). So `memory-templates.md:257`'s "rendered into `.codescout/system-prompt.md`" is aspirational, not implemented.

## Evidence
### The injection reads the root file
`src/prompts/mod.rs` (`build_server_instructions`):
```rust
if let Some(prompt) = &status.system_prompt {
    instructions.push_str("\n\n## Custom Instructions\n\n");
    instructions.push_str(prompt);
    instructions.push('\n');
}
```
`status.system_prompt` is populated by `project_status()` from the **root** file only.

### The write goes to memories/
`MemoryStore::topic_path` (`src/memory/mod.rs:123-126`) joins `self.memories_dir` — and `resolve_memory_dir` for a non-private topic resolves to `.codescout/memories/`. No branch routes `topic == "system-prompt"` to the project root.

### The refresh path already does it correctly (asymmetry)
`build_prompt_refresh_subagent_prompt` (`src/prompts/builders.rs:648-708`), step 5 (`:701`): **"Write the updated content to `.codescout/system-prompt.md`"** — a direct root-file write (not `memory(write)`). So *refreshing* an onboarded project fixes the root file, but *initial* onboarding never created it. This asymmetry is why the symptom is intermittent: refreshed projects look fine; freshly-onboarded ones don't.

### The completion message mis-reports success
`src/prompts/source.md:343-344`: "**System prompt** (`.codescout/system-prompt.md`) — always-on project guidance, injected into every session." Printed right after onboarding wrote the content to the *memories* dir, so the statement is false for a just-onboarded project.

## Proposed fix
Pick one canonical write path and make onboarding match the injection's read path. Recommended: **option 2.**

1. **Special-case the memory write** so `memory(action="write", topic="system-prompt")` also renders to the root `.codescout/system-prompt.md`. Makes `memory-templates.md:257` true with minimal prompt churn, but pollutes `MemoryStore` semantics (one topic escaping `memories/`) and leaves a duplicate file.
2. **(Recommended) Change onboarding to write the root file directly**, exactly as the refresh path already does (`builders.rs:701`). Update `memory-templates.md:255-275`, `workspace_onboarding_prompt.md:211-212`, and `builders.rs:909` to instruct a direct write to `.codescout/system-prompt.md` (e.g. `create_file`/`edit_file`) instead of `memory(write, topic="system-prompt")`. One canonical artifact (the root file), aligned with both the injection and `source.md:343`'s stated contract. Drop the `system-prompt` *memory topic* convention (it is not the always-on surface).
3. **Make the injection fall back to the memory file** (`project_status` reads `.codescout/memories/system-prompt.md` when the root is absent). Treats the memory as canonical — but contradicts `source.md`'s "memories = read on demand" model and keeps two files.

Also fix the false doc claims regardless of option: `memory-templates.md:257` ("rendered into `.codescout/system-prompt.md`") and `workspace_onboarding_prompt.md:211` ("write to `.codescout/system-prompt.md` via `memory(write)`").

No `ONBOARDING_VERSION` bump is needed for prose-only instruction fixes unless `build_system_prompt_draft()` output changes (per `onboarding.rs:16-26`).

## Tests to add
- **Prompt-builder negative guard** (mirrors the `!contains("project=")` guard from the `project=`/`project_id=` bug): the onboarding/synthesis system-prompt instruction must NOT emit `memory(action="write", topic="system-prompt")` (or must emit a root-file write), so a regression fails the build. Targets `build_synthesis_prompt` and the single-project path in `src/tools/run_command/tests.rs`.
- **Round-trip test:** simulate onboarding's system-prompt write, then assert `project_status().system_prompt` (root file) is `Some(non_empty)` and that `build_server_instructions` emits a populated `## Custom Instructions`. Currently fails (content lands in `memories/`).

## Workarounds
- Re-run `onboarding()` on the already-onboarded project to trigger the refresh subagent (`builders.rs:701`), which writes the root file correctly; **or**
- Manually copy the content: `cp .codescout/memories/system-prompt.md .codescout/system-prompt.md` (the stopgap used in claude-plugins this session).

## Fix

Resolved via **Option 2** (write the root file directly), 2026-06-12.

Onboarding (single-project `source.md` onboarding_prompt surface), workspace
synthesis (`build_synthesis_prompt`), the workspace Phase 5 instruction
(`workspace_onboarding_prompt.md`), and the shared `workspace-scope: system-prompt`
template (`memory-templates.md`) now instruct a direct `create_file` write to
`.codescout/system-prompt.md` instead of `memory(action="write", topic="system-prompt")`.
The false "rendered into root" doc claims are corrected. `ONBOARDING_VERSION`
bumped 28→29 so already-onboarded projects regenerate and write the root file on
next connect (self-heals existing broken projects, not just new onboards).

Regression guards (`src/prompts/mod.rs`):
`onboarding_prompts_write_system_prompt_to_root_not_memory` and
`synthesis_prompt_writes_system_prompt_to_root_not_memory` — assert the
`create_file` instruction is present and no affirmative
`memory(write, topic="system-prompt", content=...)` remains. Full lib suite
(2690) + clippy green; onboarding snapshot regenerated; byte-for-byte slice and
2200-byte server_instructions cap gates unaffected.

Shipped: experiments `8427ae4a` (sibling description-budget fix `31a655e5`).
Master cherry-pick pending Docs Lotus Frog review per CLAUDE.md git workflow.
Closed: 2026-06-12.
## References
- `src/prompts/mod.rs` — `build_server_instructions` appends `status.system_prompt` as `## Custom Instructions`.
- `src/server.rs:103`, `:478` — computes/re-sends `server_instructions` from `project_status()`.
- `src/agent/mod.rs:1101-1125` — `project_status()` reads the root file; TOML fallback at `:1115`.
- `src/memory/mod.rs:73-80` (`MemoryStore::write`), `:123-126` (`topic_path`) — routes to `.codescout/memories/`, no `system-prompt` special case.
- `src/prompts/source.md:~325` (instruction route), `:343-344` (completion message).
- `src/prompts/memory-templates.md:255-275` — `workspace-scope: system-prompt` template (the "rendered into root" claim).
- `src/prompts/workspace_onboarding_prompt.md:211-212` — explicit false claim.
- `src/prompts/builders.rs:909` (`build_synthesis_prompt`), `:648-708`/`:701` (`build_prompt_refresh_subagent_prompt`, the correct direct write).
- `src/config/project.rs:36` — `project.toml` `system_prompt` deprecation.
- Related: `docs/issues/2026-06-09-onboarding-prompt-uses-project-not-project-id.md` — same family (onboarding prompt text vs. live tool semantics).
