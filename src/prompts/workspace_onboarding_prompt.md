# WORKSPACE MODE — Multi-Project Onboarding

> **This section replaces the single-project Phase 1 and Phase 2 below** when multiple projects are detected. Follow these phases instead.

You are onboarding a **multi-project workspace**. The projects are listed in the "Discovered projects" table above.

---

## Phase 1A — Breadth-First Workspace Survey

Before deep-diving any single project, get a high-level understanding of ALL projects.

**For each project in the workspace:**
1. `list_dir("{project_root}")` — see top-level structure
2. `list_symbols` on the main source directory
3. `read_file` the build config (Cargo.toml / package.json / go.mod)
4. Note: purpose, tech stack, size (rough file count), key entry points

**Write a Workspace Exploration Summary** before proceeding. Include:
- One-sentence description of each project
- How projects relate to each other (which depends on which)
- Shared infrastructure (CI, tooling, deployment)
- Key cross-project patterns

> <HARD-GATE>: Do NOT proceed to Phase 1B until you have written the Workspace Exploration Summary above. This summary is required input for the subagent prompts in Phase 1B.

---

## Phase 1B — Subagent Deep Dives

Dispatch one Agent subagent per project using the template below. Set `run_in_background: true` for all subagents so they run in parallel.

**Subagent prompt template** (fill in placeholders for each project):

```
You are deep-diving the "{project_id}" project in a multi-project workspace.

## Workspace Context
{paste your Workspace Exploration Summary here}

## Your Assignment
Deep-dive the `{project_root}/` directory and write 3 per-project memories:
1. `project-overview` — purpose, tech stack, key deps, runtime requirements
2. `architecture` — module structure, key abstractions, data flow, patterns
3. `conventions` — language/framework-specific patterns, naming, testing

## Sibling Projects (for context, do NOT deep-dive these)
{list other projects with 1-sentence descriptions}

## How to Write Memories
Use: `memory(action: "write", project: "{project_id}", topic: "...", content: "...")`

## Exploration Steps (scoped to {project_root}/)
1. `list_dir("{project_root}")` — see structure
2. `list_symbols` on ALL source files in the project
3. `read_file` on build config, README if present
4. `find_symbol(include_body=true)` on 3-5 key functions/types
5. `semantic_search` for 3+ concepts specific to this project
6. Read test files to understand testing patterns
7. **Record search scope examples** in your `architecture` or `conventions` memory:
   - 3–5 good `semantic_search(query, project: "{project_id}")` query examples
     that are specific enough to return only results from THIS project
   - 1–2 query terms that are too broad (would return false-positives from
     sibling projects) and should always be scoped with `project: "{project_id}"`

## Rules
- Be specific: file paths, function names, concrete patterns
- Do NOT document sibling project internals — note dependencies only
- 15-40 lines per memory
- When you encounter types from sibling projects, note them as
  "imports FooType from {sibling}" but do not document FooType itself
```

**Re-onboarding variant**: If `per_project_protected_memories` is present in the response, include that state in the subagent prompt so it can check staleness and decide what to update.

**Failure handling**: If a subagent fails or times out, note the failure, proceed with remaining projects, and inform the user which projects need re-onboarding.

---

## Phase 2 — Workspace Memory Synthesis

After all subagent deep dives complete, read back per-project memories and write 5 workspace-level memories.

**Read per-project memories first:**
- `memory(action: "read", project: "{id}", topic: "architecture")` for each project
- `memory(action: "read", project: "{id}", topic: "conventions")` for each project

**Then write these 5 workspace-level memories** (no `project:` parameter = workspace-level):

#### `architecture` (workspace-level)
```
# Workspace Architecture

## Project Map
- {project_id}/ — {1-sentence purpose} (see `memory(project: "{id}", topic: "architecture")`)

## Cross-Project Dependencies
{project_a} → {project_b} ({what is shared})

## Shared Infrastructure
[CI, deployment, shared tooling]
```

#### `conventions` (workspace-level)
```
# Workspace Conventions

## Shared
[Commit style, PR process, CI rules, monorepo-wide patterns]

## Per-Project
[For each project: "see `memory(project: "{id}", topic: "conventions")`"]
```

#### `development-commands` — shared dev commands (build, test, lint for the whole repo)
#### `domain-glossary` — terms used across multiple projects
#### `gotchas` — cross-project pitfalls, dependency mismatches, known issues

**Write `system-prompt.md`** using `memory(action: "write", topic: "system-prompt", content: "...")` with the system prompt draft as the content.

**Single confirmation pass**: After writing all memories, confirm with the user: list all memories written and ask if anything needs adjustment.

---

## Re-Onboarding Flow

When `onboarding(force: true)` is called:

1. **Detect project state changes** using `per_project_protected_memories`:
   - `exists: false` → new project, needs full deep dive
   - `stale_files: [...]` → project changed, needs re-onboarding
   - Fresh + exists → skip (up to date)

2. **Dispatch subagents only for new + stale projects**

3. **Removed projects**: If a project directory no longer exists but has memories, inform the user and suggest `memory(action: "delete", project: "{id}", topic: "...")` for cleanup.

4. **Workspace memory merge**: After subagent updates, re-run Phase 2 synthesis but merge changes rather than overwriting — note what changed.
