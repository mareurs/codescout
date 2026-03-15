# Workspace-Aware Onboarding Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the onboarding flow workspace-aware so multi-project repos get per-project memories and workspace-level synthesis, while single-project repos remain unchanged.

**Architecture:** The onboarding prompt (`onboarding_prompt.md`) gains conditional workspace sections. `build_onboarding_prompt()` is refactored to accept an `OnboardingContext` struct. `Onboarding::call()` writes programmatic memories per project and includes per-project protected memory state. `call_content()` and `format_onboarding()` surface workspace fields to the LLM. A new `workspace_onboarding_prompt.md` file contains the Phase 1A/1B/2 instructions for multi-project repos.

**Tech Stack:** Rust, serde_json, MemoryStore, existing workspace infrastructure

**Spec:** `docs/superpowers/specs/2026-03-14-workspace-onboarding-design.md`

---

## Chunk 1: Foundation — test helpers, OnboardingContext, workspace prompt

### Task 1: Create `project_ctx_at` test helper and single-project regression test

Multiple tasks need a `ToolContext` for an arbitrary directory. The existing `project_ctx()` creates its own tempdir internally. We need a variant that accepts a path.

**Files:**
- Modify: `src/tools/workflow.rs` (test helpers section, around `project_ctx()` at line ~1987)

- [ ] **Step 1: Read the existing `project_ctx()` helper**

Read `src/tools/workflow.rs` around line 1987 to understand the pattern. The new helper `project_ctx_at(root: &Path)` should do the same setup but use the given path instead of creating a tempdir.

- [ ] **Step 2: Write `project_ctx_at` helper**

Add below the existing `project_ctx()`:

```rust
/// Like project_ctx() but uses the given directory as the project root.
/// Caller is responsible for keeping the tempdir alive.
async fn project_ctx_at(root: &std::path::Path) -> ToolContext {
    // Same body as project_ctx() but using `root` instead of `dir.path()`
    // and NOT creating a tempdir.
    // [adapt from project_ctx — use Agent::new(root.to_path_buf(), ...) etc.]
}
```

Also add a shared workspace setup helper to reduce duplication:

```rust
/// Create a two-project workspace layout in the given directory.
/// Returns (api_dir, web_dir).
fn setup_workspace_dirs(root: &std::path::Path) -> (PathBuf, PathBuf) {
    let api_dir = root.join("api");
    std::fs::create_dir_all(api_dir.join("src")).unwrap();
    std::fs::write(api_dir.join("Cargo.toml"), "[package]\nname = \"api\"").unwrap();
    std::fs::write(api_dir.join("src/main.rs"), "fn main() {}").unwrap();
    let web_dir = root.join("web");
    std::fs::create_dir_all(web_dir.join("src")).unwrap();
    std::fs::write(
        web_dir.join("package.json"),
        r#"{"name":"web","scripts":{"build":"tsc"}}"#,
    ).unwrap();
    std::fs::write(web_dir.join("src/index.ts"), "console.log('hello')").unwrap();
    (api_dir, web_dir)
}
```

- [ ] **Step 3: Write single-project regression test**

This ensures the existing flow is not broken by our refactoring:

```rust
#[tokio::test]
async fn single_project_onboarding_unchanged() {
    let (_dir, ctx) = project_ctx().await;
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    // Single project: no workspace_mode field or it's false
    assert!(
        result.get("workspace_mode").is_none()
            || result["workspace_mode"] == false
    );
    // Instructions should contain the standard Phase 1/Phase 2, not workspace phases
    let instructions = result["instructions"].as_str().unwrap_or("");
    assert!(instructions.contains("Phase 1: Explore the Code"));
    assert!(instructions.contains("Phase 2: Write the 6 Memories"));
    assert!(!instructions.contains("Phase 1A"));
    assert!(!instructions.contains("Workspace Survey"));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test single_project_onboarding_unchanged -- --nocapture`
Expected: PASS (nothing changed yet, this is a baseline).

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "test: add project_ctx_at helper, workspace setup helper, and single-project regression test"
```

---

### Task 2: Create workspace onboarding prompt file and constant

Task 1 (OnboardingContext refactor) needs `WORKSPACE_ONBOARDING_PROMPT` to exist. Create it first so Task 3 can reference it without a compile-time dependency.

**Files:**
- Create: `src/prompts/workspace_onboarding_prompt.md`
- Modify: `src/prompts/mod.rs` (add constant)

- [ ] **Step 1: Add `WORKSPACE_ONBOARDING_PROMPT` constant**

In `src/prompts/mod.rs`, after the `ONBOARDING_PROMPT` constant (line 75), add:

```rust
/// Workspace-specific onboarding prompt — appended when multiple projects are discovered.
pub const WORKSPACE_ONBOARDING_PROMPT: &str = include_str!("workspace_onboarding_prompt.md");
```

- [ ] **Step 2: Create `src/prompts/workspace_onboarding_prompt.md`**

This file contains the workspace-specific phases. The entire content below goes inside the file (NOT in a code fence — it IS the file):

The file should contain these sections:
- **WORKSPACE MODE — Multi-Project Onboarding** — intro explaining this replaces single-project Phase 1/Phase 2
- **Phase 1A — Breadth-First Workspace Survey** — instructions to survey ALL projects before deep dives, with a `<HARD-GATE>` requiring a Workspace Exploration Summary before proceeding
- **Phase 1B — Subagent Deep Dives** — instructions to dispatch one Agent per project with `run_in_background: true`. Include a complete **subagent prompt template** with placeholders: `{project_id}`, `{project_root}`, workspace context, sibling descriptions, memory write instructions (`memory(action: "write", project: "{project_id}", ...)`), exploration steps scoped to the project, memory templates for `project-overview`/`architecture`/`conventions`, rules about not documenting sibling internals. Include re-onboarding variant with protected memory state. Include failure handling: note failures, proceed, inform user.
- **Phase 2 — Workspace Memory Synthesis** — instructions to read back per-project memories, then write 5 workspace-level memories (`architecture`, `conventions`, `development-commands`, `domain-glossary`, `gotchas`) with templates showing how to reference per-project memories (e.g., `"see memory(project: 'api', topic: 'architecture')"`). Write `system-prompt.md`. Single confirmation pass.
- **Re-Onboarding Flow** — detect new/removed/stale/fresh projects, dispatch subagents only for new+stale, inform user about removed projects suggesting manual cleanup, workspace memories go through normal merge flow.

Key content for the subagent template section:

```
You are deep-diving the "{project_id}" project in a multi-project workspace.

## Workspace Context
{paste your workspace exploration summary here}

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

## Rules
- Be specific: file paths, function names, concrete patterns
- Do NOT document sibling project internals — note dependencies only
- 15-40 lines per memory
- When you encounter types from sibling projects, note them as
  "imports FooType from {sibling}" but do not document FooType itself
```

Key content for workspace memory templates section:

```
#### `architecture` (workspace-level)
# Workspace Architecture

## Project Map
- {project_id}/ — {1-sentence purpose} (see `memory(project: "{id}", topic: "architecture")`)

## Cross-Project Dependencies
{project_a} → {project_b} ({what is shared})

## Shared Infrastructure
[CI, deployment, shared tooling]

#### `conventions` (workspace-level)
# Workspace Conventions

## Shared
[Commit style, PR process, CI rules, monorepo-wide patterns]

## Per-Project
[For each project: "see `memory(project: "{id}", topic: "conventions")`"]
```

- [ ] **Step 3: Write test for workspace prompt content**

```rust
#[test]
fn workspace_onboarding_prompt_contains_key_sections() {
    assert!(WORKSPACE_ONBOARDING_PROMPT.contains("Phase 1A"));
    assert!(WORKSPACE_ONBOARDING_PROMPT.contains("Phase 1B"));
    assert!(WORKSPACE_ONBOARDING_PROMPT.contains("Phase 2"));
    assert!(WORKSPACE_ONBOARDING_PROMPT.contains("Subagent"));
    assert!(WORKSPACE_ONBOARDING_PROMPT.contains("HARD-GATE"));
    assert!(WORKSPACE_ONBOARDING_PROMPT.contains("Re-Onboarding"));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test workspace_onboarding_prompt_contains_key_sections -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/prompts/workspace_onboarding_prompt.md src/prompts/mod.rs
git commit -m "feat(onboarding): add workspace onboarding prompt for multi-project repos"
```

---

### Task 3: Introduce `OnboardingContext` struct and refactor `build_onboarding_prompt`

**Files:**
- Modify: `src/prompts/mod.rs:79-147` (refactor `build_onboarding_prompt`)
- Modify: `src/tools/workflow.rs` (update call site in `Onboarding::call()`)

**Depends on:** Task 2 (needs `WORKSPACE_ONBOARDING_PROMPT` constant to exist).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn onboarding_prompt_includes_workspace_projects() {
    use std::path::PathBuf;
    let projects = vec![
        crate::workspace::DiscoveredProject {
            id: "api".to_string(),
            relative_root: PathBuf::from("api"),
            languages: vec!["rust".to_string()],
            manifest: Some("Cargo.toml".to_string()),
        },
        crate::workspace::DiscoveredProject {
            id: "frontend".to_string(),
            relative_root: PathBuf::from("frontend"),
            languages: vec!["typescript".to_string()],
            manifest: Some("package.json".to_string()),
        },
    ];
    let ctx = OnboardingContext {
        languages: &["rust".to_string(), "typescript".to_string()],
        top_level: &["api/".to_string(), "frontend/".to_string()],
        key_files: &[],
        ci_files: &[],
        entry_points: &["api/src/main.rs".to_string()],
        test_dirs: &[],
        index_ready: false,
        index_files: 0,
        index_chunks: 0,
        projects: &projects,
        is_workspace: true,
    };
    let prompt = build_onboarding_prompt(&ctx);
    assert!(prompt.contains("Workspace"));
    assert!(prompt.contains("Phase 1A"));
    assert!(prompt.contains("api"));
    assert!(prompt.contains("frontend"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test onboarding_prompt_includes_workspace_projects -- --nocapture`
Expected: FAIL — `OnboardingContext` does not exist.

- [ ] **Step 3: Define `OnboardingContext` and refactor**

In `src/prompts/mod.rs`:

```rust
/// Context for building the onboarding prompt.
pub struct OnboardingContext<'a> {
    pub languages: &'a [String],
    pub top_level: &'a [String],
    pub key_files: &'a [String],
    pub ci_files: &'a [String],
    pub entry_points: &'a [String],
    pub test_dirs: &'a [String],
    pub index_ready: bool,
    pub index_files: usize,
    pub index_chunks: usize,
    pub projects: &'a [crate::workspace::DiscoveredProject],
    pub is_workspace: bool,
}
```

Change `build_onboarding_prompt` to accept `&OnboardingContext`. Keep all existing body logic (replacing `languages` with `ctx.languages`, etc.). At the end, before the "Gathered Project Data" section, add:

```rust
if ctx.is_workspace && ctx.projects.len() > 1 {
    prompt.push_str(&format!(
        "**Workspace mode:** {} projects detected\n\n",
        ctx.projects.len()
    ));
    prompt.push_str(WORKSPACE_ONBOARDING_PROMPT);
    prompt.push_str("\n\n");
    prompt.push_str("**Discovered projects:**\n\n");
    prompt.push_str("| Project | Root | Languages | Build |\n");
    prompt.push_str("|---------|------|-----------|-------|\n");
    for p in ctx.projects {
        prompt.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            p.id,
            p.relative_root.display(),
            p.languages.join(", "),
            p.manifest.as_deref().unwrap_or("-"),
        ));
    }
    prompt.push('\n');
}
```

- [ ] **Step 4: Update call site in `Onboarding::call()`**

In `src/tools/workflow.rs`, around line 1105, replace the `build_onboarding_prompt()` call:

```rust
let is_workspace = gathered.projects.len() > 1;
let prompt = crate::prompts::build_onboarding_prompt(&crate::prompts::OnboardingContext {
    languages: &lang_list,
    top_level: &top_level,
    key_files: &key_files,
    ci_files: &gathered.ci_files,
    entry_points: &gathered.entry_points,
    test_dirs: &gathered.test_dirs,
    index_ready: index_status["ready"].as_bool().unwrap_or(false),
    index_files: index_status["files"].as_u64().unwrap_or(0) as usize,
    index_chunks: index_status["chunks"].as_u64().unwrap_or(0) as usize,
    projects: &gathered.projects,
    is_workspace,
});
```

- [ ] **Step 5: Update existing tests using `build_onboarding_prompt`**

Find all test call sites in `src/prompts/mod.rs` and update to use `OnboardingContext` with `projects: &[]` and `is_workspace: false`.

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All pass including new workspace test and existing regression test from Task 1.

- [ ] **Step 7: Fmt + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: Clean.

- [ ] **Step 8: Commit**

```bash
git add src/prompts/mod.rs src/tools/workflow.rs
git commit -m "refactor: introduce OnboardingContext for build_onboarding_prompt"
```

---

## Chunk 2: Per-project programmatic memories and protected state

### Task 4: Write programmatic memories per project

**Files:**
- Modify: `src/tools/workflow.rs` (`Onboarding::call()` memory writing section, ~line 1040-1060)

**Depends on:** Task 1 (needs `project_ctx_at` and `setup_workspace_dirs` helpers).

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn onboarding_writes_per_project_programmatic_memories() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    setup_workspace_dirs(root);

    let ctx = project_ctx_at(root).await;
    Onboarding.call(json!({}), &ctx).await.unwrap();

    // Per-project memory directories should exist with onboarding + language-patterns
    let api_mem = root.join(".codescout/projects/api/memories");
    assert!(api_mem.join("onboarding.md").exists(), "api onboarding memory missing");
    assert!(api_mem.join("language-patterns.md").exists(), "api language-patterns missing");
    let web_mem = root.join(".codescout/projects/web/memories");
    assert!(web_mem.join("onboarding.md").exists(), "web onboarding memory missing");
    assert!(web_mem.join("language-patterns.md").exists(), "web language-patterns missing");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test onboarding_writes_per_project_programmatic_memories -- --nocapture`
Expected: FAIL — per-project memory directories not created.

- [ ] **Step 3: Add per-project programmatic memory writing**

In `Onboarding::call()`, after the existing `p.memory.write("onboarding", &summary)` block (~line 1050), add:

```rust
// Write programmatic memories for each sub-project in workspace mode.
// We compute paths directly from root + project info, since the workspace
// may not be loaded yet during first onboarding.
if gathered.projects.len() > 1 {
    for project in &gathered.projects {
        let mem_dir = if project.relative_root == PathBuf::from(".") {
            root.join(".codescout").join("memories")
        } else {
            root.join(".codescout")
                .join("projects")
                .join(&project.id)
                .join("memories")
        };
        if let Ok(store) = crate::memory::MemoryStore::from_dir(mem_dir) {
            let proj_summary = format!(
                "Languages: {}\nRoot: {}\nManifest: {}",
                project.languages.join(", "),
                project.relative_root.display(),
                project.manifest.as_deref().unwrap_or("none"),
            );
            let _ = store.write("onboarding", &proj_summary);
            if let Some(patterns) = build_language_patterns_memory(&project.languages) {
                let _ = store.write("language-patterns", &patterns);
            }
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test onboarding_writes_per_project_programmatic_memories -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): write programmatic memories per sub-project"
```

---

### Task 5: Add `workspace_mode` flag and per-project protected memory state

**Files:**
- Modify: `src/tools/workflow.rs` (`Onboarding::call()` response JSON, `gather_protected_memory_state`)

**Depends on:** Task 4.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn onboarding_includes_workspace_mode_and_per_project_protected() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    setup_workspace_dirs(root);

    let ctx = project_ctx_at(root).await;
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    assert_eq!(result["workspace_mode"], true);
    assert!(result["per_project_protected_memories"].is_object());
    // Each discovered project should have an entry
    let ppm = &result["per_project_protected_memories"];
    assert!(ppm["api"].is_object(), "api protected state missing");
    assert!(ppm["web"].is_object(), "web protected state missing");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test onboarding_includes_workspace_mode_and_per_project_protected -- --nocapture`
Expected: FAIL — `workspace_mode` not in response.

- [ ] **Step 3: Add workspace fields to response JSON**

In `Onboarding::call()`, before building the final `Ok(json!({...}))`, add:

```rust
// Per-project protected memory state for workspace mode.
// Uses the root-level protected topics config — sub-projects inherit it.
// (Per-project config override is a future enhancement.)
let (workspace_mode, per_project_protected) = if gathered.projects.len() > 1 {
    let protected = ctx
        .agent
        .with_project(|p| Ok(p.config.memory.protected.clone()))
        .await
        .unwrap_or_default();
    let mut map = serde_json::Map::new();
    for project in &gathered.projects {
        let mem_dir = if project.relative_root == PathBuf::from(".") {
            root.join(".codescout").join("memories")
        } else {
            root.join(".codescout")
                .join("projects")
                .join(&project.id)
                .join("memories")
        };
        let project_root = root.join(&project.relative_root);
        if let Ok(store) = crate::memory::MemoryStore::from_dir(mem_dir.clone()) {
            let state = gather_protected_memory_state(
                &store, &mem_dir, &project_root, &protected,
            );
            map.insert(project.id.clone(), state);
        }
    }
    (true, Some(Value::Object(map)))
} else {
    (false, None)
};
```

Then in the final JSON object, add:

```rust
"workspace_mode": workspace_mode,
"per_project_protected_memories": per_project_protected.unwrap_or(json!(null)),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test onboarding_includes_workspace_mode_and_per_project_protected -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add workspace_mode and per-project protected memory state"
```

---

## Chunk 3: LLM-facing output and system prompt

### Task 6: Update `call_content` and `format_onboarding` for workspace mode

**Files:**
- Modify: `src/tools/workflow.rs:1157-1196` (`call_content`) and `src/tools/workflow.rs:1340-1353` (`format_onboarding`)

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn onboarding_call_content_includes_workspace_info() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    setup_workspace_dirs(root);

    let ctx = project_ctx_at(root).await;
    let content = Onboarding.call_content(json!({}), &ctx).await.unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");

    assert!(
        text.contains("workspace") || text.contains("Workspace"),
        "call_content should mention workspace mode, got: {}",
        &text[..text.len().min(200)]
    );
    assert!(
        text.contains("Phase 1A") || text.contains("Workspace Survey"),
        "call_content should include workspace instructions"
    );
    // Per-project protected memories should be surfaced
    assert!(
        text.contains("per_project_protected") || text.contains("Per-Project Protected"),
        "call_content should surface per-project protected memory state"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test onboarding_call_content_includes_workspace_info -- --nocapture`
Expected: FAIL.

- [ ] **Step 3: Update `format_onboarding`**

Replace the function body at `src/tools/workflow.rs:1340-1353`:

```rust
fn format_onboarding(result: &Value) -> String {
    let langs = result["languages"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "?".to_string());
    let created = result["config_created"].as_bool().unwrap_or(false);
    let config_note = if created { " · config created" } else { "" };
    let workspace_note = if result["workspace_mode"].as_bool().unwrap_or(false) {
        let count = result["projects"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0);
        format!(" · workspace ({count} projects)")
    } else {
        String::new()
    };
    format!("[{langs}]{config_note}{workspace_note}")
}
```

- [ ] **Step 4: Update `call_content` for workspace mode**

In the full-onboarding path of `call_content` (~line 1180), after the existing `features_suggestion` append, add:

```rust
// Surface per-project protected memory state in workspace mode
if val["workspace_mode"].as_bool().unwrap_or(false) {
    if let Some(ppm) = val.get("per_project_protected_memories") {
        if !ppm.is_null() {
            response.push_str("\n\n## Per-Project Protected Memories\n\n");
            response.push_str(
                &serde_json::to_string_pretty(ppm).unwrap_or_default()
            );
        }
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test onboarding_call_content_includes_workspace_info -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Run full suite + clippy**

Run: `cargo test && cargo fmt && cargo clippy -- -D warnings`
Expected: Clean.

- [ ] **Step 7: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): workspace-aware call_content and format_onboarding"
```

---

### Task 7: Add per-project memory references to system prompt draft

**Files:**
- Modify: `src/tools/workflow.rs:661-700` (`build_system_prompt_draft`, workspace projects table)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn system_prompt_draft_includes_per_project_memory_refs() {
    use std::path::PathBuf;
    let projects = vec![
        crate::workspace::DiscoveredProject {
            id: "api".to_string(),
            relative_root: PathBuf::from("api"),
            languages: vec!["rust".to_string()],
            manifest: Some("Cargo.toml".to_string()),
        },
        crate::workspace::DiscoveredProject {
            id: "web".to_string(),
            relative_root: PathBuf::from("web"),
            languages: vec!["typescript".to_string()],
            manifest: Some("package.json".to_string()),
        },
    ];
    let draft = build_system_prompt_draft(
        &["rust".to_string(), "typescript".to_string()],
        &[],
        None,
        Some(&projects),
        &Vec::new(), // empty LibraryEntry vec
    );
    assert!(draft.contains("memory(project:"), "should reference per-project memories");
    assert!(draft.contains("api"), "should mention api project");
    assert!(draft.contains("web"), "should mention web project");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test system_prompt_draft_includes_per_project_memory_refs -- --nocapture`
Expected: FAIL — no `memory(project:` in current output.

- [ ] **Step 3: Add per-project memory references**

In `build_system_prompt_draft`, after the workspace projects table (around line 700, after the `Use project: "name"` line), add:

```rust
draft.push_str(
    "**Per-project details:** Use `memory(project: \"<id>\", topic: \"architecture\")` \
     or `memory(project: \"<id>\", topic: \"conventions\")` for project-specific knowledge.\n\n",
);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test system_prompt_draft_includes_per_project_memory_refs -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "feat(onboarding): add per-project memory refs to system prompt draft"
```

---

### Task 8: Update existing `onboarding_prompt.md` for workspace awareness

**Files:**
- Modify: `src/prompts/onboarding_prompt.md`

The existing prompt needs minor updates so it doesn't confuse the LLM when workspace
content is appended.

- [ ] **Step 1: Update the opening line**

Change:
```
You have just onboarded this project. Your job is to create 6 memories and a system
prompt that give future AI sessions deep, accurate knowledge of this codebase.
```
To:
```
You have just onboarded this project. Your job is to create memories and a system
prompt that give future AI sessions deep, accurate knowledge of this codebase.
For single-project repos, this means 6 memories. For multi-project workspaces,
see the WORKSPACE MODE section below (if present).
```

- [ ] **Step 2: Update Phase 2 heading to be conditional**

Change:
```
## Phase 2: Write the 6 Memories
```
To:
```
## Phase 2: Write the Memories (Single-Project Mode)
```

Add a note after the heading:
```
> **If you see a "WORKSPACE MODE" section below**, skip this section entirely and
> follow the workspace flow instead. This section applies only to single-project repos.
```

- [ ] **Step 3: Update "After Everything Is Created" for workspace awareness**

Add a workspace variant at the end of the section:

```
> **For workspace repos:** The above applies to single-project repos. For workspace repos,
> the subagent deep dives + workspace synthesis flow replaces this section. Summarize
> all per-project and workspace-level memories in one confirmation pass.
```

- [ ] **Step 4: Verify existing prompt test still passes**

Run: `cargo test onboarding_prompt_contains_key_sections -- --nocapture`
Expected: PASS (we haven't removed any required sections).

- [ ] **Step 5: Commit**

```bash
git add src/prompts/onboarding_prompt.md
git commit -m "docs(onboarding): add workspace awareness to existing onboarding prompt"
```

---

### Task 9: Integration test — full workspace onboarding flow

**Files:**
- Modify: `src/tools/workflow.rs` (add integration test)

- [ ] **Step 1: Write the integration test**

```rust
#[tokio::test]
async fn workspace_onboarding_full_flow() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    setup_workspace_dirs(root);

    let ctx = project_ctx_at(root).await;

    // First onboarding
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    // Workspace mode active
    assert_eq!(result["workspace_mode"], true);
    assert!(result["projects"].as_array().unwrap().len() >= 2);

    // Per-project programmatic memories written
    assert!(root.join(".codescout/projects/api/memories/onboarding.md").exists());
    assert!(root.join(".codescout/projects/web/memories/onboarding.md").exists());

    // workspace.toml created
    assert!(root.join(".codescout/workspace.toml").exists());

    // Instructions contain workspace sections
    let instructions = result["instructions"].as_str().unwrap();
    assert!(instructions.contains("Workspace"), "instructions should contain workspace content");
    assert!(instructions.contains("Phase 1A"), "instructions should contain Phase 1A");

    // System prompt draft references per-project memories
    let draft = result["system_prompt_draft"].as_str().unwrap();
    assert!(draft.contains("api"));
    assert!(draft.contains("web"));
    assert!(draft.contains("memory(project:"));

    // call_content delivers workspace content
    let content = Onboarding.call_content(json!({ "force": true }), &ctx).await.unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(text.contains("workspace") || text.contains("Workspace"));

    // Single-project regression: the workspace test above passes, now verify
    // format_compact shows workspace info
    let compact = Onboarding.format_compact(&result).unwrap_or_default();
    assert!(compact.contains("workspace"));
}
```

- [ ] **Step 2: Run the integration test**

Run: `cargo test workspace_onboarding_full_flow -- --nocapture`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "test: add workspace onboarding integration test"
```

---

### Task 10: Final cleanup, prompt surface check, and release build

- [ ] **Step 1: Check prompt surface consistency**

Grep all three prompt surfaces for workspace references:

```bash
grep -l "workspace\|Workspace" src/prompts/server_instructions.md src/prompts/onboarding_prompt.md src/prompts/workspace_onboarding_prompt.md
```

Verify `server_instructions.md` mentions workspace/project scoping if needed (it should
already have the `project` parameter documented from the workspace commit). If not,
add a brief note about workspace-aware memory routing.

- [ ] **Step 2: Run the full quality gate**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: All clean.

- [ ] **Step 3: Build release binary**

Run: `cargo build --release`
Expected: Clean build.

- [ ] **Step 4: Commit any remaining cleanup**

```bash
git add -A && git commit -m "chore: workspace onboarding cleanup and prompt surface check"
```

---

## Summary

| Task | What | Files | Depends on |
|------|------|-------|------------|
| 1 | Test helpers + single-project regression | `workflow.rs` (tests) | — |
| 2 | Workspace onboarding prompt file | `workspace_onboarding_prompt.md`, `mod.rs` | — |
| 3 | `OnboardingContext` struct + refactor | `mod.rs`, `workflow.rs` | Task 2 |
| 4 | Per-project programmatic memories | `workflow.rs` | Task 1 |
| 5 | `workspace_mode` + per-project protected state | `workflow.rs` | Task 4 |
| 6 | `call_content` + `format_onboarding` | `workflow.rs` | Task 5 |
| 7 | System prompt draft per-project refs | `workflow.rs` | Task 3 |
| 8 | Update existing `onboarding_prompt.md` | `onboarding_prompt.md` | Task 2 |
| 9 | Integration test | `workflow.rs` | Tasks 1-8 |
| 10 | Cleanup + prompt surface check + release | — | Task 9 |

**Independent parallelizable pairs:** Tasks 1 & 2 can run in parallel. Tasks 4 & 7 can run in parallel (different code sections). Task 8 is independent of Tasks 4-7.
