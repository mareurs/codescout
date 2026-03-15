# Workspace-Aware Onboarding

## Status
**Proposed** — 2026-03-14

## Problem

The workspace commit (d40c4e7) added full multi-project infrastructure: per-project
memory directories (`.codescout/projects/{id}/memories/`), `project` parameter on the
memory tool, `resolve_memory_dir()` routing, project discovery, and workspace.toml
auto-creation. However, the onboarding prompt (`onboarding_prompt.md`) has **zero
awareness of workspaces**. The `Onboarding::call()` method writes `onboarding` and
`language-patterns` memories only to the focused project's store, and the LLM receives
no guidance about exploring or writing memories for sub-projects.

Result: workspace repos get onboarded as if they were a single project. Per-project
memory directories exist but remain empty. The infrastructure is unused.

## Approach: Hybrid Workspace + Per-Project Memories

**Single-project repos** use the existing flow unchanged — no overhead added.

**Multi-project repos** (when `discover_projects()` finds >1 project) switch to a
three-phase flow:

1. **Breadth-first survey** (main session) — survey all projects at structure level
2. **Targeted deep dives** (parallel subagents) — one per project, writes per-project memories
3. **Workspace synthesis** (main session) — writes workspace-level memories that reference per-project ones

This is progressive discoverability applied to memories: workspace memories are the
compact overview, per-project memories are the focused detail.

## Memory Layout

| Memory | Single-project | Workspace: per-project | Workspace: root |
|--------|---------------|----------------------|-----------------|
| `project-overview` | root | each subproject | — |
| `architecture` | root | each subproject | cross-project map + refs |
| `conventions` | root | each subproject | shared rules + refs |
| `development-commands` | root | — | workspace-level |
| `domain-glossary` | root | — | workspace-level |
| `gotchas` | root | — | workspace-level |
| `system-prompt.md` | root | — | one file, workspace-level |
| `onboarding` | root (programmatic) | each (programmatic) | — |
| `language-patterns` | root (programmatic) | each (programmatic) | — |

**Storage paths:**
- Per-project: `.codescout/projects/{id}/memories/{topic}.md`
- Workspace (root): `.codescout/memories/{topic}.md`

## Design

### Phase 0 — Mostly Unchanged

Embedding model selection and semantic index check work at workspace level. The semantic
index is stored in `.codescout/embeddings.db` at the workspace root and already tags
chunks with `project_id` (from the workspace commit). Indexing covers all projects in
a single pass — no per-project indexing step is needed. The LLM's Phase 0 prompt
instructions do not change for workspace mode.

### Memory Count

In workspace mode, the "6 memories" language from the single-project flow does not
apply directly. Instead:
- **Per-project (subagent):** 3 memories (`project-overview`, `architecture`, `conventions`)
  plus 2 programmatic (`onboarding`, `language-patterns`)
- **Workspace-level (main session):** 5 memories (`architecture`, `conventions`,
  `development-commands`, `domain-glossary`, `gotchas`) plus `system-prompt.md`

The onboarding prompt must use conditional phrasing for workspace vs single-project.

### Phase 1A — Breadth-First Workspace Survey (Main Session)

The main session surveys all discovered projects at structure level:

- `list_dir` on each project root
- `list_symbols` on each project's entry points
- `read_file` on each project's build config (Cargo.toml, package.json, etc.)
- Identify languages, key abstractions, and relative complexity per project
- Map cross-project relationships (shared deps, imports between projects)

**Output:** A workspace exploration summary covering:
- What each project does (1-2 sentences)
- Languages and frameworks per project
- Relative complexity assessment (which projects need deep dives)
- Cross-project relationships and dependency direction

**Gate:** Must have surveyed every project's structure before proceeding to Phase 1B.

### Phase 1B — Targeted Deep Dives (Parallel Subagents)

The main session dispatches one subagent per project for deep exploration. Subagents
run in parallel.

**Subagent context (provided by main session):**

1. **Workspace survey results** — all project IDs, roots, languages, relationships
   from Phase 1A
2. **Project assignment** — which project to deep-dive, which memories to write
   (`project-overview`, `architecture`, `conventions`)
3. **Memory templates** — same templates from the onboarding prompt, so quality is
   consistent across projects
4. **Cross-project context** — what sibling projects do, so the subagent understands
   boundaries (e.g., "shared-lib provides common types used by api/ and frontend/")
5. **Protected memory state** — if re-onboarding, existing per-project memories and
   their staleness status

**Subagent work:**

1. Explore the assigned project following Phase 1 exploration steps (scoped to that
   project's directory)
2. Write 3 per-project memories via `memory(action: "write", project: "{id}", topic: "...")`:
   - `project-overview` — purpose, stack, deps, runtime requirements
   - `architecture` — module structure, key abstractions, data flow, patterns
   - `conventions` — language/framework-specific patterns, naming, testing
3. The `Onboarding::call()` method writes programmatic `onboarding` and
   `language-patterns` memories for each project automatically
4. Return a summary to the main session: what was written, key findings, notable
   gotchas discovered

**Subagents write memories autonomously** — no per-subagent user confirmation. The main
session does a single confirmation pass at the end.

**Subagent dispatch mechanism:**
- The LLM dispatches subagents using Claude Code's native Agent tool with
  `subagent_type: "general-purpose"` and `run_in_background: true`
- Multiple subagents are dispatched in a single message (parallel launch)
- Each subagent's prompt includes the full workspace survey context, memory templates,
  and explicit instructions to call `memory(action: "write", project: "{id}", ...)`
- Results are collected when each background agent completes (Claude Code notifies
  the main session automatically)
- The main session waits for all subagents before proceeding to Phase 2

**Subagent failure handling:**
- If a subagent fails (timeout, LSP crash, permission error), the main session notes
  the failure and proceeds with remaining subagents
- After Phase 2, the main session informs the user which projects failed and suggests
  re-running with `onboarding(force: true)` to retry those projects
- Failed projects do not block workspace-level memory writing — the main session works
  with whatever per-project memories were successfully written

**Cross-project references in subagents:**
- When a subagent encounters types/APIs from a sibling project, it should note the
  dependency in its `architecture` memory (e.g., "imports FooType from shared-lib")
  but NOT document the sibling's internals — that's the sibling subagent's job

### Phase 2 — Workspace Memory Synthesis (Main Session)

After all subagents complete, the main session:

1. Reads back per-project memories to understand what each subagent wrote
2. Writes workspace-level memories:

   **`architecture`** — Cross-project relationships and dependency graph. References
   per-project architecture memories rather than duplicating details:
   ```
   ## Project Map
   - api/ — REST backend (see `memory(project: "api", topic: "architecture")`)
   - frontend/ — React SPA (see `memory(project: "frontend", topic: "architecture")`)
   - shared-lib/ — Common types (see `memory(project: "shared-lib", topic: "architecture")`)

   ## Cross-Project Dependencies
   api/ → shared-lib/ (imports domain types)
   frontend/ → api/ (HTTP client, generated types)
   ```

   **`conventions`** — Shared rules (commit style, CI, PR process, monorepo-wide
   patterns). References per-project conventions:
   ```
   ## Shared
   - Conventional commits required
   - CI runs on all PRs

   ## Per-Project
   - Rust conventions: see `memory(project: "api", topic: "conventions")`
   - React conventions: see `memory(project: "frontend", topic: "conventions")`
   ```

   **`development-commands`** — Workspace-level build/test/lint commands

   **`domain-glossary`** — Terms spanning the whole repo

   **`gotchas`** — Cross-cutting pitfalls (monorepo gotchas, CI quirks, inter-project issues)

3. Writes `system-prompt.md` — single entry point that includes the workspace project
   table and references per-project memories for drill-down

4. Confirms everything with the user in one pass

### Re-Onboarding (force: true) for Workspaces

When re-onboarding a workspace:

1. Main session runs breadth-first survey (Phase 1A)
2. Detects changes:
   - **New projects** — discovered but no per-project memories exist → full deep dive
   - **Removed projects** — memories exist but project no longer discovered → inform
     the user and suggest manual deletion of `.codescout/projects/{id}/` (do not
     auto-delete — the project may have moved or the discovery depth may have changed)
   - **Stale projects** — per-project memories exist but `protected_memories` shows stale
     anchors → deep dive with merge flow
   - **Fresh projects** — per-project memories exist, all anchors fresh → skip (no subagent)
3. Dispatches subagents only for new and stale projects
4. Workspace-level memories go through the normal protected memory merge flow
5. Single confirmation pass at the end

### Single-Project Repos

**No changes.** The existing flow (7 exploration steps → 6 memories + system prompt)
continues as-is. The workspace path only activates when `projects.len() > 1` in the
onboarding response.

## Implementation Surface

### Prompt Changes

**`src/prompts/onboarding_prompt.md`:**
- New conditional section after Phase 1: "Phase 1A — Workspace Survey" and
  "Phase 1B — Subagent Deep Dives" (only shown when `projects.len() > 1`)
- New Phase 2 variant for workspace memory writing (workspace-level templates)
- Subagent dispatch instructions with context template
- Updated "After Everything Is Created" section for workspace summary
- Gate checklist additions: "surveyed every project's structure"

**`src/prompts/mod.rs` — `build_onboarding_prompt()`:**
- Refactor to accept an `OnboardingContext` struct instead of 9+ positional parameters
  (current function already has `#[allow(clippy::too_many_arguments)]`)
- `OnboardingContext` includes: existing params + `projects: Vec<DiscoveredProject>`,
  `is_workspace: bool`
- Conditional inclusion of workspace-specific prompt sections
- Per-project data appended to "Gathered Project Data" section

### Code Changes

**`src/tools/workflow.rs` — `Onboarding::call()`:**
- Write programmatic `onboarding` and `language-patterns` memories for each discovered
  project (not just the focused one). Requires instantiating a `MemoryStore::from_dir()`
  for each sub-project's memory directory via `memory_dir_for_project()` — the current
  code only has a `MemoryStore` for the focused project through `ActiveProject.memory`
- Include per-project `protected_memories` state in the onboarding response (one entry
  per project)
- Add `workspace_mode: true` flag to response JSON when multi-project
- Add `per_project_protected_memories: { "api": {...}, "frontend": {...} }` to response

**`src/tools/workflow.rs` — `Onboarding::call_content()`:**
- Update to surface workspace-mode fields in the text content the LLM sees:
  `workspace_mode`, `per_project_protected_memories`, subagent dispatch instructions
- Currently inlines `instructions` and `system_prompt_draft` from the JSON — needs
  conditional workspace sections

**`src/tools/workflow.rs` — `format_onboarding()`:**
- Update compact representation to handle workspace-mode fields (`workspace_mode`,
  `per_project_protected_memories`, project count)

**`src/tools/workflow.rs` — `build_system_prompt_draft()`:**
- Add per-project memory references to the workspace projects table:
  `"Use memory(project: 'api', topic: 'architecture') for project-specific details"`

**`src/tools/workflow.rs` — `gather_protected_memory_state()`:**
- New variant that gathers state for a specific project's memory directory
- Called once per discovered project during onboarding
- Anchor paths must be resolved relative to each sub-project's root (not workspace
  root) for correct staleness detection

### No Changes Needed

- `resolve_memory_dir()` — already routes by project param
- `memory_dir_for_project()` — already handles root vs sub-project dirs
- `discover_projects()` — already works
- Memory tool `project` parameter — already wired
- `WorkspaceConfig` / `workspace.toml` — already created during onboarding
- Single-project onboarding flow — unchanged

## Edge Cases & Known Limitations

1. **Single non-root project** — If a repo has exactly 1 project at a non-root path
   (e.g., `packages/only-project/`), `projects.len() == 1` so the single-project flow
   activates. Memories go to root-level `.codescout/memories/`. This is acceptable —
   the workspace flow adds overhead that isn't justified for a single project.

2. **Nested/dominated projects** — `discover_projects` deduplicates dominated paths
   (parent subsumes child). If discovery depth or domination rules change between
   onboardings, a previously-onboarded sub-project could disappear. The re-onboarding
   "removed projects" flow handles this by informing the user rather than auto-deleting.

3. **Phase 1A is prompt-driven** — The breadth-first survey in Phase 1A is entirely
   LLM behavior guided by the onboarding prompt. No new Rust code executes during
   Phase 1A — the LLM calls existing tools (`list_dir`, `list_symbols`, `read_file`).

4. **Parallel memory writes are safe** — Subagents write to disjoint per-project
   directories. `resolve_memory_dir` acquires a read lock on `agent.inner` (not write),
   so concurrent subagent requests through the MCP server do not contend.

## Decisions

1. **Single-project repos unchanged** — workspace logic only activates for >1 project
2. **Hybrid memory split** — `project-overview`, `architecture`, `conventions` are
   per-project; `development-commands`, `domain-glossary`, `gotchas` are workspace-level
3. **Breadth-first then targeted depth** — main session surveys all, subagents deep-dive
4. **Subagents write autonomously** — single confirmation pass at the end, not per-subagent
5. **Selective re-onboarding** — only dispatch subagents for new/stale projects
6. **Workspace memories reference per-project ones** — progressive discoverability for
   memories; no duplication between levels
