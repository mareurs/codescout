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

<!-- TASK 3 will append workspace-scope sections below this line -->
