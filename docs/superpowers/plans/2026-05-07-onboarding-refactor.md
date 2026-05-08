# Onboarding Prompt Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract memory templates into a shared `memory-templates.md`, restructure `workspace_onboarding_prompt.md` into 6 numbered phases with read-back verification, slim `onboarding_prompt.md` to use the include, and bump `ONBOARDING_VERSION`.

**Architecture:** A new `src/prompts/memory-templates.md` is the single source of truth for the 7 memory definitions. Both prompts contain `{{include: memory-templates.md}}` markers, substituted at compile time by a tiny Rust loader (`include_str!` + `String::replace`). Workspace flow gains Phase 2 (stale-project cleanup), Phase 4 (coverage verification + retry), Phase 6 (CLAUDE.md refresh).

**Tech Stack:** Rust 2024, `include_str!` macro, existing prompt-surface test infrastructure.

**Reference spec:** `docs/superpowers/specs/2026-05-07-onboarding-refactor-design.md`

---

## File Structure

| File | Action | Purpose |
|---|---|---|
| `src/prompts/memory-templates.md` | **Create** | 7 memory definitions × 2 scopes; canonical empty-stub |
| `src/prompts/mod.rs` | Modify | `INCLUDE_MARKER`, `load_prompt()`, 4 new tests |
| `src/prompts/source.md` | Modify | Replace inline `### Memories to Create` with include marker; add STABLE-HEADING comment to Phase 0; add Coverage Verification section; rewrite Refresh CLAUDE.md section |
| `src/prompts/workspace_onboarding_prompt.md` | Modify | Renumber into 6 phases; add Stale-Project Cleanup, Coverage Verification, CLAUDE.md Refresh; require 6-memory subagent set; add include marker |
| `src/tools/onboarding.rs` | Modify | Bump `ONBOARDING_VERSION` |
| `docs/TODO-tool-misbehaviors.md` | Append | Document the audited bugs (audit log requirement) |

Each task ends with a commit. Run `cargo fmt && cargo clippy -- -D warnings && cargo test` before each commit.

---

### Task 1: Add `load_prompt()` helper and `INCLUDE_MARKER`

**Files:**
- Modify: `src/prompts/mod.rs:126-129` (add INCLUDE_MARKER, replace constants with helper output)
- Test: `src/prompts/mod.rs` tests module

- [ ] **Step 1: Write the failing test**

Append to the `tests` module in `src/prompts/mod.rs` (after the existing `workspace_onboarding_prompt_contains_key_sections` test, around line 322):

```rust
    #[test]
    fn load_prompt_substitutes_include_marker() {
        let single = load_prompt("onboarding_prompt.md");
        let workspace = load_prompt("workspace_onboarding_prompt.md");
        assert!(!single.contains("{{include: memory-templates.md}}"),
            "include marker should be substituted in single-project prompt");
        assert!(!workspace.contains("{{include: memory-templates.md}}"),
            "include marker should be substituted in workspace prompt");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout --lib prompts::tests::load_prompt_substitutes_include_marker`
Expected: FAIL — `load_prompt` not defined.

- [ ] **Step 3: Add the helper and constants**

Replace lines 126–129 of `src/prompts/mod.rs`:

```rust
pub const INCLUDE_MARKER: &str = "{{include: memory-templates.md}}";

const RAW_ONBOARDING_PROMPT: &str = include_str!("onboarding_prompt.md");
const RAW_WORKSPACE_ONBOARDING_PROMPT: &str = include_str!("workspace_onboarding_prompt.md");
const MEMORY_TEMPLATES: &str = include_str!("memory-templates.md");

/// Load a prompt with `{{include: memory-templates.md}}` markers substituted.
pub fn load_prompt(name: &str) -> String {
    let raw = match name {
        "onboarding_prompt.md" => RAW_ONBOARDING_PROMPT,
        "workspace_onboarding_prompt.md" => RAW_WORKSPACE_ONBOARDING_PROMPT,
        other => panic!("unknown prompt: {other}"),
    };
    raw.replace(INCLUDE_MARKER, MEMORY_TEMPLATES)
}

/// Backwards-compat: code that wants the substituted prompt directly.
pub static ONBOARDING_PROMPT: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| load_prompt("onboarding_prompt.md"));
pub static WORKSPACE_ONBOARDING_PROMPT: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| load_prompt("workspace_onboarding_prompt.md"));
```

- [ ] **Step 4: Update existing call sites**

Existing references to `ONBOARDING_PROMPT` and `WORKSPACE_ONBOARDING_PROMPT` are now `LazyLock<String>`, not `&str`. Find and dereference:

```bash
grep -n "ONBOARDING_PROMPT\|WORKSPACE_ONBOARDING_PROMPT" src/prompts/mod.rs src/prompts/builders.rs src/tools/onboarding.rs
```

For each match outside `mod.rs`, replace usages like `prompts::ONBOARDING_PROMPT` with `prompts::ONBOARDING_PROMPT.as_str()` or `&*prompts::ONBOARDING_PROMPT`. Keep tests in `mod.rs` referencing the constants directly with `&*`.

- [ ] **Step 5: Create empty `memory-templates.md` so `include_str!` resolves**

```bash
echo "# Memory Templates (placeholder — filled in Task 2)" > src/prompts/memory-templates.md
```

- [ ] **Step 6: Add the include marker to both prompts (one line each)**

`src/prompts/source.md` — append at end of file:
```
<!-- TASK 5 will move this marker to the right location -->
{{include: memory-templates.md}}
```

`src/prompts/workspace_onboarding_prompt.md` — append at end of file:
```
<!-- TASK 6 will move this marker to the right location -->
{{include: memory-templates.md}}
```

- [ ] **Step 7: Run test to verify it passes**

Run: `cargo test -p codescout --lib prompts::tests::load_prompt_substitutes_include_marker`
Expected: PASS.

- [ ] **Step 8: Run full test suite + clippy + fmt**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add src/prompts/mod.rs src/prompts/memory-templates.md src/prompts/source.md src/prompts/workspace_onboarding_prompt.md
git commit -m "refactor(prompts): add load_prompt() helper and include marker

Introduces INCLUDE_MARKER + load_prompt() that substitutes
{{include: memory-templates.md}} at compile time. ONBOARDING_PROMPT
and WORKSPACE_ONBOARDING_PROMPT become LazyLock<String>.
memory-templates.md is a placeholder; populated in next task."
```

---

### Task 2: Populate `memory-templates.md` — project-scope sections

**Files:**
- Modify: `src/prompts/memory-templates.md` (replace placeholder)
- Test: `src/prompts/mod.rs` tests module

- [ ] **Step 1: Write the failing test**

Append to `tests` module in `src/prompts/mod.rs`:

```rust
    #[test]
    fn memory_templates_have_all_project_scope_sections() {
        let templates = include_str!("memory-templates.md");
        for topic in ["project-overview", "architecture", "conventions",
                      "development-commands", "domain-glossary", "gotchas"] {
            let heading = format!("### project-scope: {topic}");
            assert!(templates.contains(&heading),
                "memory-templates.md missing heading: {heading}");
        }
    }

    #[test]
    fn memory_templates_define_empty_stub() {
        let templates = include_str!("memory-templates.md");
        assert!(templates.contains("EMPTY_STUB:"),
            "memory-templates.md must define the canonical empty stub");
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p codescout --lib prompts::tests::memory_templates_have_all_project_scope_sections prompts::tests::memory_templates_define_empty_stub`
Expected: both FAIL.

- [ ] **Step 3: Replace `memory-templates.md` content with project-scope sections**

Overwrite `src/prompts/memory-templates.md` with:

````markdown
# Memory Templates

Canonical definitions for codescout memories. Referenced by both
`onboarding_prompt.md` (single-project) and `workspace_onboarding_prompt.md`
(multi-project).

## Empty-Stub Convention

For `domain-glossary` and `gotchas` with nothing project-specific to record,
write the canonical empty stub instead of skipping the memory:

```
EMPTY_STUB:
> **Status (2026-05-08): paths retargeted post-I-01.** This plan was drafted on 2026-05-07. The next day, I-01 (commits `7db51a5`..`f047d47`) consolidated `src/prompts/source.md` and `src/prompts/source.md` into a single `src/prompts/source.md` with `<!-- @surface NAME -->` blocks; build.rs slices them into `OUT_DIR/{surface}.md` at compile time. Path references below have been mechanically updated. **Two derived consequences the executor still needs to handle:**
>
> 1. **`{{include: memory-templates.md}}` substitution must run at compile time inside build.rs** (or as a pre-source.md-slice pass), so that the resulting OUT_DIR/onboarding_prompt.md and OUT_DIR/workspace_onboarding_prompt.md already contain the expanded memory templates. Either teach build.rs to do `String::replace("{{include: memory-templates.md}}", &fs::read_to_string("src/prompts/memory-templates.md")?)` before slicing, or implement `load_prompt()` as a runtime helper that reads source.md once and applies substitution per call. Pick one — the existing `pub const &str` semantics depend on it being available at static-init time, so build-time is simpler.
> 2. **Task 6 (cross-prompt invariant test)** is partially obsolete: `prompt_surfaces_reference_only_real_tools` was redirected to scan runtime constants in I-01 Phase 3 (commit `82701e2`). Re-derive against the current `src/server.rs` test, not the include_str!-based shape described below.
>
> Original design intent and the 7 audit-found bugs being fixed are intact. Path references resolve. Substitution-mechanism + test-plumbing surgery need re-derivation.

---
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
````

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p codescout --lib prompts::tests::memory_templates_have_all_project_scope_sections prompts::tests::memory_templates_define_empty_stub`
Expected: both PASS.

- [ ] **Step 5: Run full test suite + clippy + fmt**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add src/prompts/memory-templates.md src/prompts/mod.rs
git commit -m "feat(prompts): add project-scope memory templates

Six project-scope memory definitions with required subsections and
empty-stub convention. Referenced by both onboarding prompts via
{{include: memory-templates.md}}."
```

---

### Task 3: Append workspace-scope sections to `memory-templates.md`

**Files:**
- Modify: `src/prompts/memory-templates.md`
- Test: `src/prompts/mod.rs` tests module

- [ ] **Step 1: Write the failing test**

Append to `tests` module:

```rust
    #[test]
    fn memory_templates_have_all_workspace_scope_sections() {
        let templates = include_str!("memory-templates.md");
        for topic in ["architecture", "conventions", "development-commands",
                      "domain-glossary", "gotchas", "system-prompt"] {
            let heading = format!("### workspace-scope: {topic}");
            assert!(templates.contains(&heading),
                "memory-templates.md missing heading: {heading}");
        }
    }

    #[test]
    fn workspace_architecture_template_has_required_subsections() {
        let templates = include_str!("memory-templates.md");
        // Workspace architecture is the load-bearing change — must have these
        for sub in ["Project Map", "Cross-Project Dependencies",
                    "Shared Infrastructure", "Top-Level Code Map",
                    "Generic Navigation"] {
            assert!(templates.contains(&format!("- `## {sub}`")),
                "workspace architecture template missing required subsection: {sub}");
        }
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p codescout --lib prompts::tests::memory_templates_have_all_workspace_scope_sections prompts::tests::workspace_architecture_template_has_required_subsections`
Expected: both FAIL.

- [ ] **Step 3: Replace the trailing comment in `memory-templates.md` with workspace sections**

Replace `<!-- TASK 3 will append workspace-scope sections below this line -->` and everything after with:

````markdown
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
dev startup scripts, school switching). Per-project commands stay in each
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

Purpose: the per-project system prompt rendered into `.codescout/system-prompt.md`.
Generated from the prompt-builder logic, not authored directly.

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
````

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p codescout --lib prompts::tests::memory_templates_have_all_workspace_scope_sections prompts::tests::workspace_architecture_template_has_required_subsections`
Expected: both PASS.

- [ ] **Step 5: Run full test suite + clippy + fmt**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add src/prompts/memory-templates.md src/prompts/mod.rs
git commit -m "feat(prompts): add workspace-scope memory templates

Six workspace-scope templates. workspace architecture grows to five
required subsections (adds Top-Level Code Map + Generic Navigation)
to absorb root-layer monorepo content."
```

---

### Task 4: Refactor `workspace_onboarding_prompt.md` into 6 phases

**Files:**
- Modify: `src/prompts/workspace_onboarding_prompt.md` (full rewrite)
- Test: `src/prompts/mod.rs` tests module

- [ ] **Step 1: Write the failing test**

Append to `tests` module:

```rust
    #[test]
    fn workspace_prompt_has_six_phases() {
        let workspace = load_prompt("workspace_onboarding_prompt.md");
        for phase in [
            "## Phase 1 — Workspace Survey",
            "## Phase 2 — Stale-Project Cleanup",
            "## Phase 3 — Per-Project Deep Dives",
            "## Phase 4 — Coverage Verification",
            "## Phase 5 — Workspace Synthesis",
            "## Phase 6 — CLAUDE.md Refresh",
        ] {
            assert!(workspace.contains(phase),
                "workspace prompt missing phase: {phase}");
        }
    }

    #[test]
    fn workspace_prompt_requires_six_memories_per_project() {
        let workspace = load_prompt("workspace_onboarding_prompt.md");
        assert!(workspace.contains("6 memories"),
            "workspace subagent prompt must require 6 memories per project");
        for topic in ["project-overview", "architecture", "conventions",
                      "development-commands", "domain-glossary", "gotchas"] {
            assert!(workspace.contains(topic),
                "workspace prompt missing topic name: {topic}");
        }
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p codescout --lib prompts::tests::workspace_prompt_has_six_phases prompts::tests::workspace_prompt_requires_six_memories_per_project`
Expected: both FAIL.

- [ ] **Step 3: Overwrite `src/prompts/workspace_onboarding_prompt.md` with new structure**

Full content (replace entire file):

````markdown
# WORKSPACE MODE — Multi-Project Onboarding

You are onboarding a **multi-project workspace**. The projects are listed
in the "Discovered projects" table that follows this prompt.

> Single-project Phase 1 / Phase 2 instructions do NOT apply here — this
> file is the complete flow. Do NOT spawn a "root" exploration subagent;
> root-layer content is absorbed into workspace `architecture` (Top-Level
> Code Map + Generic Navigation) per the included memory templates.

---

{{include: memory-templates.md}}

---

## Phase 1 — Workspace Survey

Breadth-first sweep of all projects before any deep dive.

For each project in the workspace:
1. `tree("{project_root}")` — top-level structure
2. `symbols` on the main source directory
3. `read_file` the build config (Cargo.toml / package.json / go.mod)
4. Note: purpose, tech stack, rough file count, key entry points

**Write a Workspace Exploration Summary** before proceeding:
- One-sentence description of each project
- Cross-project dependencies
- Shared infrastructure (CI, deployment, tooling)
- Key cross-project patterns

<HARD-GATE>
Do NOT proceed to Phase 2 until the Workspace Exploration Summary is
written. The summary is required input for Phase 3 subagent prompts.
</HARD-GATE>

---

## Phase 2 — Stale-Project Cleanup

**Runs only when `onboarding(force: true)`.** Skip this phase otherwise.

1. List `.codescout/projects/*/` directories.
2. Intersect with the live project list from Phase 1.
3. Orphans = directories not in the live set.
4. For each orphan, read the most-recent mtime under `<orphan>/memories/`.

If any orphans found, ask the user **once**:

```
Found N orphaned codescout projects (no longer in workspace):
  - <id>   last touched <date>
  - <id>   last touched <date>

Delete all? [y/N]
```

- On `y`: run `memory(action: "delete", project_id: "<id>", topic: "<topic>")`
  for every memory file under each orphan, then remove the empty
  `<project>/memories/` directory.
- On `N` or no answer: log `cleanup: skipped (N orphans retained)` for the
  final summary. Do NOT re-ask in subsequent phases.

<HARD-GATE>
Phase 2 is complete when the user has answered the cleanup prompt OR no
orphans were found. Record the outcome for the final summary.
</HARD-GATE>

---

## Phase 3 — Per-Project Deep Dives

Dispatch one Agent subagent per project **in a single parallel batch** —
all Agent tool calls in one response turn.

**Subagent prompt template** (fill placeholders for each project):

```
You are deep-diving the "{project_id}" project in a multi-project workspace.

## Workspace Context
{paste the Workspace Exploration Summary from Phase 1}

## Your Assignment

Write all 6 project-scope memories defined in memory-templates.md:
1. project-overview
2. architecture
3. conventions
4. development-commands
5. domain-glossary
6. gotchas

For domain-glossary and gotchas with nothing project-specific to record,
write the empty stub from memory-templates.md (do NOT skip).

Use: memory(action: "write", project_id: "{project_id}", topic: "<topic>", content: "...")

## Sibling Projects (for context, do NOT deep-dive these)
{list other projects with 1-sentence descriptions}

## Exploration Steps (scoped to {project_root}/)
1. tree("{project_root}") — structure
2. symbols on ALL source files in the project
3. read_file on build config, README if present
4. symbols(name=..., include_body=true) on 3–5 key functions/types
5. semantic_search for 3+ concepts specific to this project
6. Read test files to understand testing patterns

## Rules
- Be specific: file paths, function names, concrete patterns
- Do NOT document sibling project internals — note dependencies only
- 15–40 lines per memory (or empty stub for eligible topics)
- When you encounter types from sibling projects, note them as
  "imports FooType from {sibling}" but do not document FooType itself

## End-of-Run Manifest
End your final response with a single line listing exactly what you wrote:

  MANIFEST: project-overview, architecture, conventions, development-commands, domain-glossary, gotchas

The line is advisory — the parent verifies by reading back. Topics MUST
exactly match the 6 above (use empty-stub when nothing to say).
```

<HARD-GATE>
Do NOT proceed to Phase 4 until every Agent subagent has **returned**
(not just been dispatched). If a subagent fails or times out, note the
failure but proceed to Phase 4 — the read-back will catch it.
</HARD-GATE>

---

## Phase 4 — Coverage Verification

For each `(project_id, topic)` pair (6 × N projects), run:

```
memory(action: "read", project_id: "<id>", topic: "<topic>")
```

Build a coverage matrix with three states per cell:

- **✓** — content present, longer than the empty stub.
- **EMPTY** — content byte-equal to the canonical empty stub from
  memory-templates.md. Counts as ✓ for `domain-glossary` and `gotchas`
  only. For other topics, treat EMPTY as MISSING.
- **MISSING** — read returns "not found" / null / error / content shorter
  than the stub.

If the matrix has any MISSING cells, run the **retry loop** (max 2 attempts):

```
attempt = 0
while attempt < 2 and matrix has MISSING:
    failed_projects = projects with at least one MISSING cell
    re-dispatch ONE subagent per failed project with prompt:

        "Re-onboarding: {project_id}.
         Previous run was incomplete. You wrote: {present_topics}.
         Missing: {missing_topics}.
         Constraints:
           - Do NOT re-read sibling projects.
           - Do NOT rewrite topics already present.
           - For empty-stub eligible topics with nothing to say, write the
             stub exactly:
             {paste EMPTY_STUB block from memory-templates.md}
         End with: MANIFEST: {missing_topics}"

    wait for all subagents to return
    re-read missing cells
    attempt += 1
```

If the matrix still has MISSING after the retry budget is exhausted, abort:

```
Onboarding aborted at Phase 4 — coverage incomplete after retries:

  <project>   missing: [<topic>, <topic>]

Workspace synthesis and CLAUDE.md refresh skipped.
Re-run /onboarding force to retry just the failed projects.
```

Skip Phase 5 and Phase 6 on abort.

<HARD-GATE>
Phase 4 is complete only when the coverage matrix is 100% green
(✓ or eligible-EMPTY for every cell). Otherwise, abort.
</HARD-GATE>

---

## Phase 5 — Workspace Synthesis

Read per-project memories that feed cross-project synthesis:
- `memory(action: "read", project_id: "<id>", topic: "architecture")` for each project
- `memory(action: "read", project_id: "<id>", topic: "conventions")` for each project
- (Read other topics only when needed for cross-project content.)

Then write the 6 workspace-scope memories from `memory-templates.md`:
- `architecture` (with all 5 required subsections — Project Map,
  Cross-Project Dependencies, Shared Infrastructure, Top-Level Code Map,
  Generic Navigation)
- `conventions`
- `development-commands`
- `domain-glossary` (empty stub if no cross-project terms)
- `gotchas` (empty stub if no cross-project pitfalls)
- `system-prompt` — write to `.codescout/system-prompt.md` via
  `memory(action: "write", topic: "system-prompt", content: ...)`

After writing, read each back and verify:
- 5 markdown memories non-empty (or eligible-EMPTY)
- system-prompt file exists and contains its required subsections

<HARD-GATE>
Phase 5 is complete only when all 6 workspace memories are written and
verified. On failure, retry once; if still failing, abort to user.
</HARD-GATE>

---

## Phase 6 — CLAUDE.md Refresh

Compute the canonical memory table from what was actually written this run.
Each row's "What's inside" cell is the first `## H2` of the memory body
(empty-stub memories show `_no project-specific entries_`).

Read existing `CLAUDE.md`. Locate `## codescout Memories`:
- If present: compute a unified diff for the table block.
- If absent: propose adding a new `## codescout Memories` section.

Print the diff and ask the user **once**:

```
Proposed CLAUDE.md memory-table update:

  [unified diff, ~10 lines]

Apply? [y/N]
```

- On `y`: `edit_markdown(path: "CLAUDE.md", action: "replace", heading: "## codescout Memories", content: <new table>)`
- On `N` or no answer: log `claude_md: skipped (user declined)` to the
  final summary. Do NOT re-ask.

<HARD-GATE>
Phase 6 is complete when the user has answered. No follow-up questions.
</HARD-GATE>

---

## Final Summary

Print exactly one block, no chatter:

```
Onboarding complete.

  Per-project memories
    <id>          6/6 ✓
    <id>          6/6 ✓ (1 retry)

  Workspace memories     6/6 ✓

  Cleanup                <N orphans deleted | skipped | none>
  CLAUDE.md              <updated | skipped>
  System prompt          v<old> → v<new>
```

Skipped/failed phases show `✗ <reason>` instead of `✓`.
````

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p codescout --lib prompts::tests::workspace_prompt_has_six_phases prompts::tests::workspace_prompt_requires_six_memories_per_project`
Expected: both PASS.

- [ ] **Step 5: Update existing tests that referenced old workspace prompt headings**

`cargo test` will likely fail `workspace_onboarding_prompt_contains_key_sections` (around line 315). Open `src/prompts/mod.rs` and replace the test body to match the new headings:

```rust
    #[test]
    fn workspace_onboarding_prompt_contains_key_sections() {
        let prompt = &*WORKSPACE_ONBOARDING_PROMPT;
        assert!(prompt.contains("# WORKSPACE MODE"));
        assert!(prompt.contains("## Phase 1 — Workspace Survey"));
        assert!(prompt.contains("## Phase 3 — Per-Project Deep Dives"));
        assert!(prompt.contains("## Phase 4 — Coverage Verification"));
        assert!(prompt.contains("## Phase 5 — Workspace Synthesis"));
        assert!(prompt.contains("## Phase 6 — CLAUDE.md Refresh"));
    }
```

- [ ] **Step 6: Run full test suite + clippy + fmt**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add src/prompts/workspace_onboarding_prompt.md src/prompts/mod.rs
git commit -m "refactor(prompts): restructure workspace onboarding into 6 phases

Adds Phase 2 (stale-project cleanup, force-only), Phase 4 (coverage
verification + retry loop, max 2 attempts), Phase 6 (CLAUDE.md
refresh with diff/ask flow). Per-project subagents now write 6
memories with empty-stub for domain-glossary/gotchas. Subagent
manifest is advisory only — read-back is the gate."
```

---

### Task 5: Refactor `onboarding_prompt.md` (single-project)

**Files:**
- Modify: `src/prompts/source.md`
- Test: `src/prompts/mod.rs` tests module

- [ ] **Step 1: Write the failing test**

Append to `tests` module:

```rust
    #[test]
    fn onboarding_prompt_uses_include_marker() {
        // The raw file (pre-substitution) must have the marker
        let raw = include_str!("onboarding_prompt.md");
        assert!(raw.contains("{{include: memory-templates.md}}"),
            "onboarding_prompt.md must contain the include marker");
        // After load_prompt, marker is replaced by template content
        let loaded = load_prompt("onboarding_prompt.md");
        assert!(!loaded.contains("{{include:"));
        assert!(loaded.contains("### project-scope: project-overview"));
    }

    #[test]
    fn onboarding_prompt_phase_0_has_stable_heading_marker() {
        let raw = include_str!("onboarding_prompt.md");
        assert!(raw.contains("STABLE-HEADING"),
            "Phase 0 must carry a STABLE-HEADING comment to prevent cross-prompt drift");
    }
```

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p codescout --lib prompts::tests::onboarding_prompt_uses_include_marker prompts::tests::onboarding_prompt_phase_0_has_stable_heading_marker`
Expected: both FAIL (placeholder content from Task 1 doesn't have these).

- [ ] **Step 3: Read the current single-project prompt to plan edits**

```bash
cat src/prompts/source.md | head -50
```

Identify and remove the inline `### Memories to Create` block (lines ~232–490 in the original file before Task 1). Replace with the include marker. Add STABLE-HEADING comment to Phase 0. Add Coverage Verification + rewritten Refresh CLAUDE.md sections.

- [ ] **Step 4: Apply edits to `onboarding_prompt.md`**

Use `edit_markdown` to make targeted section edits:

```
edit_markdown(
  path: "src/prompts/source.md",
  action: "edit",
  heading: "## Phase 0: Embedding Model Selection",
  old_string: "## Phase 0: Embedding Model Selection",
  new_string: "<!-- STABLE-HEADING: workspace_onboarding_prompt.md may reference this section by exact title. Do not rename without updating cross-references. -->\n## Phase 0: Embedding Model Selection"
)
```

Then replace the entire `### Memories to Create` block + 7 numbered memory subsections (`### 1.` through `### 7.`) with the include marker. Use `edit_markdown` with `action: "replace"` and `include_subsections: true`:

```
edit_markdown(
  path: "src/prompts/source.md",
  action: "replace",
  heading: "### Memories to Create",
  include_subsections: true,
  content: "Apply the **project-scope** sections of the included memory templates below. Write all 6 project-scope memories. Use the empty stub for `domain-glossary` and `gotchas` if nothing project-specific applies — do NOT skip them.\n\nFor `system-prompt`, apply the `workspace-scope: system-prompt` section (single-project flow treats the project as its own workspace).\n\n{{include: memory-templates.md}}"
)
```

Remove the bare-marker line at the bottom of the file added by Task 1:

```
edit_file(
  path: "src/prompts/source.md",
  old_string: "<!-- TASK 5 will move this marker to the right location -->\n{{include: memory-templates.md}}",
  new_string: ""
)
```

Add a Coverage Verification section after `## After Everything Is Created`:

```
edit_markdown(
  path: "src/prompts/source.md",
  action: "insert_after",
  heading: "## After Everything Is Created",
  content: "## Coverage Verification\n\nAfter writing all 6 project-scope memories, read each back:\n\n```\nmemory(action: \"read\", topic: \"<topic>\")\n```\n\nVerify each is present (or matches the canonical empty stub for eligible topics). If any read fails or returns content shorter than the empty stub, retry the missing write up to 2 times. If still missing, abort with a clear error and do NOT proceed to CLAUDE.md refresh.\n"
)
```

Rewrite the Refresh CLAUDE.md section to match Phase 6:

```
edit_markdown(
  path: "src/prompts/source.md",
  action: "replace",
  heading: "### Refresh CLAUDE.md",
  content: "Compute the canonical memory table from what was written this run. Each row's \"What's inside\" cell is the first `## H2` of the memory body.\n\nRead existing `CLAUDE.md`. Locate `## codescout Memories` (or propose adding it). Generate a unified diff for the table block.\n\nAsk the user **once**:\n\n```\nProposed CLAUDE.md memory-table update:\n\n  [unified diff]\n\nApply? [y/N]\n```\n\nOn `y`: `edit_markdown(path: \"CLAUDE.md\", action: \"replace\", heading: \"## codescout Memories\", content: <new table>)`. On `N` or no answer: log `claude_md: skipped (user declined)` for the final summary. No follow-up questions.\n"
)
```

- [ ] **Step 5: Run tests to verify pass**

Run: `cargo test -p codescout --lib prompts::tests::onboarding_prompt_uses_include_marker prompts::tests::onboarding_prompt_phase_0_has_stable_heading_marker`
Expected: both PASS.

- [ ] **Step 6: Update other onboarding tests that referenced old `### 1.` style headings**

Search for stale references:

```bash
grep -n "### 1\.\|### 2\.\|Memories to Create" src/prompts/mod.rs
```

If `onboarding_prompt_contains_key_sections` (line 296) checks for these, update it to assert the new include-based shape:

```rust
    #[test]
    fn onboarding_prompt_contains_key_sections() {
        let prompt = &*ONBOARDING_PROMPT;
        assert!(prompt.contains("## THE IRON LAW"));
        assert!(prompt.contains("## Phase 0: Embedding Model Selection"));
        assert!(prompt.contains("## Phase 1: Semantic Index Check"));
        assert!(prompt.contains("## Phase 2: Explore the Code"));
        assert!(prompt.contains("### project-scope: project-overview"));
        assert!(prompt.contains("### project-scope: architecture"));
        assert!(prompt.contains("## Coverage Verification"));
        assert!(prompt.contains("### Refresh CLAUDE.md"));
    }
```

- [ ] **Step 7: Run full test suite + clippy + fmt**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
Expected: all green. If `prompt_surfaces_reference_only_real_tools` complains about a stale tool name in the new content, fix the offending mention.

- [ ] **Step 8: Commit**

```bash
git add src/prompts/source.md src/prompts/mod.rs
git commit -m "refactor(prompts): slim single-project onboarding prompt

Drops ~330 lines of inline memory templates in favor of
{{include: memory-templates.md}}. Adds STABLE-HEADING comment on
Phase 0 to prevent cross-prompt drift, Coverage Verification
section, and a unified Refresh CLAUDE.md flow matching workspace
Phase 6."
```

---

### Task 6: Cross-prompt invariant test + ONBOARDING_VERSION bump + tracker entries

**Files:**
- Modify: `src/prompts/mod.rs` tests module
- Modify: `src/tools/onboarding.rs:21`
- Modify: `docs/trackers/skill-frictions.md`
- Modify: `docs/trackers/tool-usage-patterns.md`

- [ ] **Step 1: Write the cross-prompt invariant test**

Append to `tests` module in `src/prompts/mod.rs`:

```rust
    #[test]
    fn workspace_phase_0_reference_resolves() {
        let single = load_prompt("onboarding_prompt.md");
        let workspace = load_prompt("workspace_onboarding_prompt.md");
        let referenced = "## Phase 0: Embedding Model Selection";
        if workspace.contains(referenced) {
            assert!(single.contains(referenced),
                "workspace prompt references heading missing from single-project prompt");
        }
    }
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cargo test -p codescout --lib prompts::tests::workspace_phase_0_reference_resolves`
Expected: PASS (or no-op if workspace doesn't reference it).

- [ ] **Step 3: Bump `ONBOARDING_VERSION`**

Read current value:

```bash
grep "const ONBOARDING_VERSION" src/tools/onboarding.rs
```

Increment by 1. Example edit (replace `<N>` with current value, `<N+1>` with new):

```rust
pub const ONBOARDING_VERSION: u32 = <N+1>;
```

The accompanying comment should explain the bump reason — keep or update:

```rust
// Bumped 2026-05-07 for the prompt refactor: new memory shape
// (6 mandatory per project), Phase 6 CLAUDE.md flow, empty-stub convention.
```

- [ ] **Step 4: Verify the bump test passes**

Run: `cargo test -p codescout --lib tools::onboarding::tests`
Expected: existing version tests still PASS (they use relative comparisons, not absolute values).

- [ ] **Step 5: Append entries to skill-frictions tracker**

```
edit_markdown(
  path: "docs/trackers/skill-frictions.md",
  action: "insert_after",
  heading: "## `/onboarding`",
  content: "### F-NNN — workspace onboarding silently over-reported per-project memory writes\n**When:** Multi-project workspace with `force=true`. HARD-GATE only verified `project-overview` per project, allowing eduplanner-mcp to pass with 2 of 6 memories.\n**Got:** Final summary claimed 6/6 coverage; in reality some projects had 2–3 memories.\n**Fix idea (FIXED 2026-05-07):** Phase 4 Coverage Verification reads back all 6 topics per project; subagent MANIFEST line is advisory only.\n\n### F-NNN — onboarding root-layer content not captured\n**When:** Monorepo with real root-layer code (dev scripts, docker-compose, top-level scripts).\n**Got:** Workspace prompt explicitly forbade a root subagent and had no fallback to capture root content; root subagent without `project_id` polluted workspace memories.\n**Fix idea (FIXED 2026-05-07):** workspace `architecture` template grew Top-Level Code Map + Generic Navigation subsections; the no-root-subagent rule now states the reason."
)
```

If the heading `## /onboarding` doesn't exist, use `insert_before` against the next heading or `append`.

- [ ] **Step 6: Append entry to tool-usage-patterns tracker**

```
edit_markdown(
  path: "docs/trackers/tool-usage-patterns.md",
  action: "insert_before",
  heading: "## Prompt improvement candidates",
  content: "### T-NNN — workspace onboarding HARD-GATE checked one topic per project\n**Tool:** mcp__codescout__memory (read)\n**Verdict:** wrong-tool — gate logic was a single read per project; should have been a 6×N matrix.\n**Prompt gap:** workspace_onboarding_prompt.md HARD-GATE language was \"verify project-overview\" rather than \"verify all required topics\". Fixed 2026-05-07 with Phase 4 read-back loop.\n"
)
```

(Per `CLAUDE.md § Session Intelligence Trackers`, this is a librarian artifact. The body edit is the visible change; the params table is updated separately by the user/librarian. We document the analysis prose only.)

- [ ] **Step 7: Run full test suite + clippy + fmt**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
Expected: all green.

- [ ] **Step 8: Build the release binary to confirm prompts compile cleanly**

```bash
cargo build --release
```
Expected: build succeeds.

- [ ] **Step 9: Commit**

```bash
git add src/prompts/mod.rs src/tools/onboarding.rs docs/trackers/skill-frictions.md docs/trackers/tool-usage-patterns.md
git commit -m "chore(onboarding): bump version, add cross-prompt test, log tracker entries

Bumps ONBOARDING_VERSION to trigger system-prompt regeneration on
next onboarding call. Adds workspace_phase_0_reference_resolves
test catching cross-prompt heading drift. Documents the audited
bugs as fixed in skill-frictions and tool-usage-patterns trackers."
```

---

## Self-Review

**Spec coverage:**
- Decisions table → Tasks 2 (project-scope set), 3 (workspace), 4 (workspace flow), 5 (single-project flow). ✓
- File layout → Task 1 (loader + marker), Tasks 2–5 (file content). ✓
- 6-phase workspace flow → Task 4. ✓
- Empty-stub convention → Tasks 2 (definition), 4 (subagent prompt). ✓
- Coverage matrix + retry → Task 4 Phase 4. ✓
- CLAUDE.md refresh → Task 4 Phase 6 + Task 5. ✓
- Stale-project cleanup → Task 4 Phase 2. ✓
- Single-project trim + STABLE-HEADING → Task 5. ✓
- Loader (`include_str!` + replace) → Task 1. ✓
- Tests (4 listed in spec + cross-prompt) → spread across Tasks 1, 2, 3, 5, 6. ✓
- ONBOARDING_VERSION bump → Task 6. ✓
- Tracker hooks → Task 6. ✓

**Placeholder scan:** No "TBD"/"TODO"/"implement later" in normative steps. `<id>`, `<topic>`, `<N>` are intentional template placeholders, documented as such.

**Type consistency:** `INCLUDE_MARKER` and `load_prompt` defined in Task 1, used unchanged in Tasks 2–5. `MANIFEST:` line format defined in Task 4, referenced unchanged in retry prompt within the same task. Topic names (`project-overview`, etc.) are byte-identical across all tasks.

**Out of scope (matches spec):** templating engines, `root` as a project, hybrid detection, JSON manifests as gates.

---

Plan complete and saved to `docs/superpowers/plans/2026-05-07-onboarding-refactor.md`. Two execution options:

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — Execute tasks in this session using `executing-plans`, batch execution with checkpoints.

Which approach?
