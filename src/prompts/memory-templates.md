# Memory Templates

Canonical definitions for codescout memories. Referenced by both
`onboarding_prompt.md` (single-project) and `workspace_onboarding_prompt.md`
(multi-project).

## Empty-Stub Convention

For `domain-glossary` and `gotchas` with nothing project-specific to record,
write the canonical empty stub instead of skipping the memory:

```
EMPTY_STUB:
# {project_id} {Topic}

_No project-specific {topic}. See workspace {topic} memory._
```

The verification logic treats stub-equal content as a successful write.
Skipping a required memory is a failure; writing the stub is not.

## Project-Scope Memories

Apply this set when writing memories for a specific project, either as a
single-project subject or as a per-project subagent in workspace mode.
Use `memory(action: "write", project_id: "<id>", topic: "<topic>", content: "...")`.

### project-scope: project-overview

Purpose: one-page introduction. Tech stack, key dependencies, runtime
requirements, what this project is and is not.

Required subsections:
- `## Purpose` — one paragraph, what does this project do
- `## Tech Stack` — language, framework, build tool, key libraries
- `## Runtime Requirements` — versions, env vars, external services
- `## Entry Points` — main file(s) with line numbers, how to run

15–40 lines. No internal architecture details (those go in `architecture`).

```
---
manifest:
  topic: project-overview
  scope: project
  required_subsections: [Purpose, Tech Stack, Runtime Requirements, Entry Points]
```

### project-scope: architecture

Purpose: module structure, key abstractions, data flow.

Required subsections:
- `## Module Map` — directory tree with one-line purpose per file
- `## Key Abstractions` — 3–7 core types/traits with role
- `## Data Flow` — how a request/event moves through the system
- `## Search Tips` — 3–5 good `semantic_search(query, project_id: "{id}")`
  examples; 1–2 query terms too broad to use unscoped

15–40 lines. Concrete file paths, function names, no hand-waving.

```
---
manifest:
  topic: architecture
  scope: project
  required_subsections: [Module Map, Key Abstractions, Data Flow, Search Tips]
```

### project-scope: conventions

Purpose: language/framework idioms, naming, testing patterns specific to
this project.

Required subsections:
- `## Naming` — file/symbol/test naming patterns
- `## Testing` — framework, layout, fixture conventions
- `## Patterns` — language-specific idioms or anti-patterns enforced here

15–40 lines.

```
---
manifest:
  topic: conventions
  scope: project
  required_subsections: [Naming, Testing, Patterns]
```

### project-scope: development-commands

Purpose: build, test, lint, run commands for this project specifically.
Cross-project orchestration belongs in workspace `development-commands`.

Required subsections:
- `## Build`
- `## Test`
- `## Lint / Format`
- `## Run` — how to start the project locally

10–30 lines. Exact commands, no prose.

```
---
manifest:
  topic: development-commands
  scope: project
  required_subsections: [Build, Test, Lint / Format, Run]
```

### project-scope: domain-glossary

Purpose: terms used inside this project that a new contributor needs to
understand. Cross-project terms go in workspace `domain-glossary`.

Required subsections:
- `## Terms` — bullet list, `term — definition`

If the project has fewer than 3 unique terms, write the empty stub.

```
---
manifest:
  topic: domain-glossary
  scope: project
  required_subsections: [Terms]
  empty_stub_eligible: true
```

### project-scope: gotchas

Purpose: project-specific traps, surprising behaviors, dependencies that
silently break, things that bit a previous contributor.

Required subsections:
- `## Gotchas` — bullet list, each entry: situation + symptom + workaround

If the project has no gotchas worth documenting, write the empty stub.

```
---
manifest:
  topic: gotchas
  scope: project
  required_subsections: [Gotchas]
  empty_stub_eligible: true
```

## Workspace-Scope Memories

Apply this set when synthesizing workspace-level memories from per-project
data. Use `memory(action: "write", topic: "<topic>", content: "...")` —
no `project_id` parameter.

### workspace-scope: architecture

Purpose: top-level map of the multi-project workspace. Absorbs root-layer
content (dev scripts, docker-compose, generic navigation).

Required subsections:
- `## Project Map` — for each project: `<id>/ — <one-line purpose> · entry-point: <file>:<symbol> · activate: <command>`
- `## Cross-Project Dependencies` — `<a> → <b> (<what is shared>)`
- `## Shared Infrastructure` — CI workflows, deployment, shared tooling
- `## Top-Level Code Map` — root-layer scripts, docker-compose, env templates, generated artifacts (one line per file)
- `## Generic Navigation` — 5–8 bullets pointing at memories or files for: build/test/lint, common bugs, domain modeling, cross-project flows

40–80 lines. This memory is the entry point for all workspace navigation.

```
---
manifest:
  topic: architecture
  scope: workspace
  required_subsections: [Project Map, Cross-Project Dependencies, Shared Infrastructure, Top-Level Code Map, Generic Navigation]
```

### workspace-scope: conventions

Purpose: monorepo-wide conventions; per-project specifics live in each
project's `conventions` memory.

Required subsections:
- `## Shared` — commit style, PR process, CI rules
- `## Per-Project` — for each project: `see memory(project_id: "<id>", topic: "conventions")`

15–30 lines.

```
---
manifest:
  topic: conventions
  scope: workspace
  required_subsections: [Shared, Per-Project]
```

### workspace-scope: development-commands

Purpose: cross-project orchestration commands (whole-monorepo build/test,
dev startup scripts, stack switching). Per-project commands stay in each
project's `development-commands`.

Required subsections:
- `## Whole-Repo` — build/test/lint commands that span projects
- `## Dev Startup` — how to start the full stack locally
- `## Per-Project` — for each project: `see memory(project_id: "<id>", topic: "development-commands")`

15–40 lines.

```
---
manifest:
  topic: development-commands
  scope: workspace
  required_subsections: [Whole-Repo, Dev Startup, Per-Project]
```

### workspace-scope: domain-glossary

Purpose: terms shared across two or more projects. Project-private terms
stay in each project's `domain-glossary`.

Required subsections:
- `## Terms` — bullet list, `term — definition`

If no cross-project terms exist, write the empty stub.

```
---
manifest:
  topic: domain-glossary
  scope: workspace
  required_subsections: [Terms]
  empty_stub_eligible: true
```

### workspace-scope: gotchas

Purpose: cross-project pitfalls, dependency mismatches, things that surprise
contributors moving between projects.

Required subsections:
- `## Gotchas` — bullet list, each: situation + symptom + workaround

If none, empty stub.

```
---
manifest:
  topic: gotchas
  scope: workspace
  required_subsections: [Gotchas]
  empty_stub_eligible: true
```

### workspace-scope: system-prompt

Purpose: the per-project system prompt. Write it **directly** to the root file
`.codescout/system-prompt.md` with `create_file` — NOT via `memory(action:
"write", topic: "system-prompt")`. It is the always-on file injected into every
session (read by `project_status` from the project root), not a memory topic.

Required subsections:
- `## Entry Points`
- `## Key Abstractions`
- `## Search Tips`
- `## Navigation Strategy`
- `## Project Rules`

See `src/prompts/builders.rs::build_system_prompt_draft` for the generator.

```
---
manifest:
  topic: system-prompt
  scope: workspace
  required_subsections: [Entry Points, Key Abstractions, Search Tips, Navigation Strategy, Project Rules]
```
