# Prompt Efficiency Overhaul — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce token cost of codescout's two prompt surfaces (~38% for server_instructions.md, ~21% for onboarding_prompt.md) while preserving behavioral compliance.

**Architecture:** Edit two markdown prompt files in-place, add language filtering logic in `src/prompts/mod.rs`, append workflow content to the existing `doc://codescout-tool-guide` MCP resource via `long_docs()` on affected tools.

**Tech Stack:** Rust, Markdown, MCP resources

**Spec:** `docs/superpowers/specs/2026-04-14-prompt-efficiency-design.md`

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `src/prompts/server_instructions.md` | Modify | Rewrite Tool Reference, deduplicate Anti-Patterns, remove 2 workflows, remove Kotlin section |
| `src/prompts/mod.rs` | Modify | Add Kotlin constant + language-conditional injection |
| `src/prompts/onboarding_prompt.md` | Modify | Compress Phase 2 to goals + gate checklist |
| `src/tools/markdown.rs` | Modify | Add `long_docs()` for markdown editing workflow |
| `src/tools/symbol.rs` | Modify | Add `long_docs()` for dependency tracing workflow |
| Tests in `src/prompts/mod.rs` | Modify | Update section name assertions, add Kotlin filtering test |

---

### Task 1: Rewrite Tool Reference section (D1)

**Files:**
- Modify: `src/prompts/server_instructions.md` (lines 73–184, the `## Tool Reference` section)

- [ ] **Step 1: Read the current Tool Reference and categorize each line**

Classify every line in `## Tool Reference` as:
- **Schema-redundant**: parameter lists, basic "what it does" — CUT
- **Cross-tool routing**: "prefer X over Y" — KEEP
- **Gotcha**: non-obvious behavior not in schema — KEEP
- **Buffer ref table**: unique info — KEEP (lives in Output System, not here)

- [ ] **Step 2: Write the replacement section**

Replace the entire `## Tool Reference` section (lines 73–184) with a compact
"Tool Routing & Gotchas" section. Target: ~35 lines. Structure:

```markdown
## Tool Routing & Gotchas

Tool descriptions and parameters are in the MCP tool schemas — this section
covers only cross-tool routing and non-obvious behaviors.

### Source Code: Symbol Tools, Not File Tools

- **Reading source:** `list_symbols(path)` → `find_symbol(name, include_body=true)`.
  `read_file` on source returns a summary, not raw content.
- **Editing code:** `replace_symbol`, `insert_code`, `remove_symbol` for structural
  changes. `edit_file` is for imports, literals, comments, config only.
- **Markdown files:** `read_markdown` / `edit_markdown`, not `read_file` / `edit_file`.
  `edit_file` on `.md` files is gated to `edit_markdown` (except `insert="prepend"|"append"`).

### Search Routing

- **Know the name** → `find_symbol(query)` or `list_symbols(path)`
- **Know the concept** → `semantic_search(query)` then drill with symbol tools
- **Know a text pattern** → `grep(pattern)`
- **Know a filename** → `glob(pattern)`
- **All callers of X** → `find_references(symbol, path)` (not `grep`)

### Gotchas

- `rename_symbol` may corrupt string literals containing the old name — verify
  compilation after use.
- `run_command` output > 50 lines is buffered as `@cmd_*` ref. Query with
  `grep pattern @cmd_id` or `read_file("@cmd_id", start_line=N)`.
- `read_file` with `mode="complete"` bypasses buffering — only for plan files.
- `edit_file` `edits=[...]` batch mode is atomic (one write). Prefer over
  sequential single edits on the same file.

### Library Routing

Pass `scope="lib:<name>"` on `find_symbol`, `list_symbols`, `find_references`,
`semantic_search`, or `index_project` to target a registered library.
Libraries are auto-discovered when `goto_definition`/`hover` resolves outside
the project root. All read-only tools work on libraries; write tools are project-only.
```

- [ ] **Step 3: Verify the replacement doesn't break the "How to Choose" table**

The "How to Choose the Right Tool" table (lines 38–50) has overlap with the new
Search Routing subsection. Read both and confirm they're complementary, not
redundant. If redundant, remove the "How to Choose" table and fold its unique
content (the "Then drill with" column) into Search Routing.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib prompts`
Expected: PASS (the `static_instructions_contain_key_sections` test checks for
`## How to Choose the Right Tool`, `## Output System`, `## Rules` — none of
which are removed).

- [ ] **Step 5: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "refine(prompts): strip Tool Reference to routing + gotchas (D1)"
```

---

### Task 2: Deduplicate Anti-Patterns table (D2)

**Files:**
- Modify: `src/prompts/server_instructions.md` (the `## Anti-Patterns` section)

- [ ] **Step 1: Identify rows to remove**

Remove these rows (restate Iron Laws):
1. `edit_file` with multi-line old_string on source → `replace_symbol` (Iron Law #2)
2. `edit_file` to delete a function → `remove_symbol` (Iron Law #2)
3. `edit_file` to add code after → `insert_code` (Iron Law #2)
4. Native Edit/Write on source → codescout tools (Iron Law #2)
5. `run_command("cd /abs/path && cmd")` → use `cwd` param (Iron Law #3 variant)

- [ ] **Step 2: Write the replacement table**

Keep these rows (teach something new):

```markdown
## Anti-Patterns

| ❌ Never do this | ✅ Do this instead | Why |
|---|---|---|
| `run_command("jq '.key' @file_ref")` to query JSON | `read_file(path, json_path="$.key")` | Navigation params > shell buffer queries |
| `edit_file` / `create_file` to rewrite a markdown section | `edit_markdown(path, heading, action, content)` | Heading-addressed, no string matching needed |
| `read_file` on a `.md` file | `read_markdown(path)` | Heading navigation > line guessing |
| `grep("fn_name")` to find all callers | `find_references(symbol, path)` | LSP finds actual usages; regex matches comments, strings |
| `find_symbol(query="foo\|bar")` | `grep(pattern="foo\|bar")` or separate `find_symbol` calls | `find_symbol` rejects regex-like patterns |
| Repeat a broad `find_symbol` after overflow | Narrow with `path=`, `kind=`, or more specific pattern | Follow the overflow hint |
| Ignore `by_file` in overflow response | Use top file from `by_file` as `path=` filter | The hint tells you exactly where to look |
| `activate_project` for a single lookup | Pass `project_id: "<id>"` on the tool call | No state mutation, no risk of forgetting to return |
```

- [ ] **Step 3: Remove the rationalization paragraph below the table**

The paragraph starting "If you catch yourself rationalizing..." adds 2 lines that
are somewhat useful but overlap with Iron Laws framing. Remove it — Iron Laws
already set the tone.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib prompts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "refine(prompts): deduplicate Anti-Patterns against Iron Laws (D2)"
```

---

### Task 3: Move 2 workflows to MCP resource (D3)

**Files:**
- Modify: `src/prompts/server_instructions.md` (the `## Workflows` section)
- Modify: `src/tools/markdown.rs` — add `long_docs()` to `EditMarkdown` tool with the Markdown Editing workflow
- Modify: `src/tools/symbol.rs` — add `long_docs()` to `GotoDefinition` tool with the Dependency Tracing workflow

- [ ] **Step 1: Write the failing test — verify workflows appear in tool guide**

Add tests in `src/mcp_resources/tool_guide.rs`:

```rust
#[tokio::test]
async fn tool_guide_includes_markdown_editing_workflow() {
    // Build a ToolGuideProvider with an EditMarkdown tool, render it,
    // and assert the output contains "Editing a Markdown Document"
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(crate::tools::markdown::EditMarkdown),
    ];
    let p = ToolGuideProvider::new(tools);
    let bytes = p.read("doc://codescout-tool-guide").await.unwrap();
    match bytes {
        ResourceBytes::Text(t) => {
            assert!(t.contains("Editing a Markdown Document"),
                "tool guide must include the markdown editing workflow");
        }
        _ => panic!("expected text"),
    }
}

#[tokio::test]
async fn tool_guide_includes_dependency_tracing_workflow() {
    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(crate::tools::symbol::GotoDefinition),
    ];
    let p = ToolGuideProvider::new(tools);
    let bytes = p.read("doc://codescout-tool-guide").await.unwrap();
    match bytes {
        ResourceBytes::Text(t) => {
            assert!(t.contains("Dependency Tracing"),
                "tool guide must include the dependency tracing workflow");
        }
        _ => panic!("expected text"),
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test tool_guide_includes`
Expected: FAIL — `long_docs()` returns `None` for these tools currently.

- [ ] **Step 3: Add `long_docs()` to EditMarkdown**

In `src/tools/markdown.rs`, add to the `Tool` impl for `EditMarkdown`:

```rust
fn long_docs(&self) -> Option<&str> {
    Some(
        "### Workflow: Editing a Markdown Document\n\n\
         | Step | Tool | Purpose |\n\
         |------|------|---------|\n\
         | 1 | `read_markdown(path)` | Get heading map — see all sections |\n\
         | 2 | `read_markdown(path, headings=[...])` | Read target sections (one call, multiple sections) |\n\
         | 3a | `edit_markdown(path, heading, action, content)` | Whole-section: replace (body only — heading preserved), insert, remove |\n\
         | 3b | `edit_markdown(path, heading, action=\"edit\", old_string, new_string)` | Surgical: scoped string replacement within a section |\n\
         | 3c | `edit_markdown(path, edits=[...])` | Batch: multiple edits across sections, atomic |"
    )
}
```

- [ ] **Step 4: Add `long_docs()` to GotoDefinition**

In `src/tools/symbol.rs`, add to the `Tool` impl for `GotoDefinition`:

```rust
fn long_docs(&self) -> Option<&str> {
    Some(
        "### Workflow: Dependency Tracing — \"How does data flow from A to B?\"\n\n\
         | Step | Tool | Purpose |\n\
         |------|------|---------|\n\
         | 1 | `find_symbol(entry_point)` | Locate starting function |\n\
         | 2 | `goto_definition` on called functions | Follow the call chain forward |\n\
         | 3 | `hover` on parameters/return values | See resolved types at each stage |\n\
         | 4 | `find_references` at destination | Confirm which callers reach this point |"
    )
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test tool_guide_includes`
Expected: PASS

- [ ] **Step 6: Remove the two workflows from server_instructions.md**

Remove the `### Editing a Markdown Document` and
`### Dependency Tracing` subsections from the `## Workflows` section.

Add a one-liner after the remaining workflows:
```markdown
More workflows (markdown editing, dependency tracing) available via `resources/read doc://codescout-tool-guide`.
```

- [ ] **Step 7: Run all tests**

Run: `cargo test --lib prompts && cargo test --lib mcp_resources`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add src/prompts/server_instructions.md src/tools/markdown.rs src/tools/symbol.rs src/mcp_resources/tool_guide.rs
git commit -m "refine(prompts): move 2 workflows to tool-guide resource (D3)"
```

---

### Task 4: Dynamic Kotlin language filtering (D4)

**Files:**
- Modify: `src/prompts/server_instructions.md` — remove Kotlin section from static file
- Modify: `src/prompts/mod.rs` — add `KOTLIN_KNOWN_ISSUES` constant + conditional injection

- [ ] **Step 1: Write the failing test**

In `src/prompts/mod.rs` tests:

```rust
#[test]
fn build_with_kotlin_project_includes_kotlin_warnings() {
    let status = ProjectStatus {
        name: "test".into(),
        path: "/tmp/test".into(),
        languages: vec!["kotlin".into(), "java".into()],
        memories: vec![],
        has_index: false,
        system_prompt: None,
        github_enabled: false,
        workspace: None,
    };
    let result = build_server_instructions(Some(&status));
    assert!(result.contains("kotlin-lsp"),
        "Kotlin project must include Kotlin known issues");
}

#[test]
fn build_without_kotlin_excludes_kotlin_warnings() {
    let status = ProjectStatus {
        name: "test".into(),
        path: "/tmp/test".into(),
        languages: vec!["rust".into()],
        memories: vec![],
        has_index: false,
        system_prompt: None,
        github_enabled: false,
        workspace: None,
    };
    let result = build_server_instructions(Some(&status));
    assert!(!result.contains("kotlin-lsp"),
        "Non-Kotlin project must not include Kotlin known issues");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib prompts -- kotlin`
Expected: `build_without_kotlin_excludes_kotlin_warnings` FAILS (Kotlin section
is still in the static markdown).

- [ ] **Step 3: Extract Kotlin section from server_instructions.md**

Remove the entire `## Language Support — Known Issues` section (lines 287–306)
from `server_instructions.md`.

- [ ] **Step 4: Add the constant and conditional in mod.rs**

In `src/prompts/mod.rs`, after the existing constants:

```rust
/// Kotlin-specific known issues — only injected for projects with Kotlin.
const KOTLIN_KNOWN_ISSUES: &str = "\
## Language Support — Known Issues

### Kotlin (kotlin-lsp)

kotlin-lsp (JetBrains) has a **single workspace session** limitation: only one \
kotlin-lsp process can serve a given project directory at a time. If another \
codescout instance or editor is already running kotlin-lsp for the same project, \
new instances will fail with:

> \"Multiple editing sessions for one workspace are not supported yet\"

codescout detects this and fails fast with a clear error. **Workaround:** close \
the other session first, or use a single codescout instance for Kotlin projects.";
```

In `build_server_instructions()`, after the workspace topology block and before
the github check, add:

```rust
// Language-specific warnings — only injected when the project uses the language.
if status.languages.iter().any(|l| l == "kotlin") {
    instructions.push_str("\n\n");
    instructions.push_str(KOTLIN_KNOWN_ISSUES);
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib prompts`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add src/prompts/server_instructions.md src/prompts/mod.rs
git commit -m "refine(prompts): dynamic Kotlin section filtering (D4)"
```

---

### Task 5: Compress onboarding Phase 2 (D5)

**Files:**
- Modify: `src/prompts/onboarding_prompt.md` (lines 119–253, `## Phase 2: Explore the Code`)

- [ ] **Step 1: Write the replacement Phase 2**

Replace the 7 prescriptive steps (Steps 1–7, lines ~126–221) with goals +
orientation. Keep the Gate Checklist and Exploration Summary subsections intact.

```markdown
## Phase 2: Explore the Code

Your goal is to build a complete mental model of this codebase — enough to write
accurate, specific project memories in Phase 3. Use whatever tools and exploration
strategy you judge best. The gate checklist below is your hard constraint.

### Goals

- **Map the structure.** Understand the directory layout, module organization,
  and entry points. Know what lives where.
- **Understand core abstractions.** Identify the 3–5 key types/traits/classes
  that form the skeleton. Read their full implementations, not just signatures.
- **Read all architecture docs.** Completely — not skimmed. If docs exist, they
  contain decisions you need for accurate memories.
- **Trace at least 2 data flows.** Follow concrete operations end-to-end through
  the code, with actual function/method names — not just "the request goes through
  the middleware layer."
- **Search by concept.** Run at least 5 semantic or keyword searches for concepts
  the codebase likely embodies (error handling, caching, authentication, etc.).
  Discover what the code does that README/docs don't mention.
- **Examine tests.** Read 2–3 test files to understand testing patterns, helpers,
  and fixtures used in this project.
- **Verify the build.** Confirm the project builds and tests pass.
```

The existing `### Phase 2 Gate Checklist` and `### Exploration Summary` subsections
remain unchanged — they follow directly after.

- [ ] **Step 2: Verify line count reduction**

Run: `wc -w src/prompts/onboarding_prompt.md`
Expected: < 3500 words (was ~4329).

- [ ] **Step 3: Run tests**

Run: `cargo test --lib -- onboarding`
Expected: PASS. If any test asserts on specific Phase 2 step text (e.g.,
"Step 1: Map the Codebase Structure"), update the assertion to match the new
content or remove it if it was testing prescriptive wording.

- [ ] **Step 4: Commit**

```bash
git add src/prompts/onboarding_prompt.md
git commit -m "refine(prompts): compress onboarding Phase 2 to goals + gate (D5)"
```

---

### Task 6: Final verification and word count check

**Files:** None created — verification only.

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: ALL PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Word count verification**

Run: `wc -w src/prompts/server_instructions.md src/prompts/onboarding_prompt.md`
Expected:
- `server_instructions.md` < 1800 words (was 2765)
- `onboarding_prompt.md` < 3500 words (was 4329)

- [ ] **Step 4: Spot-check critical content survived**

Manually verify these are still present in `server_instructions.md`:
- All 5 Iron Laws (numbered, verbatim)
- Buffer ref type table (@cmd/@file/@tool/@bg)
- Impact Analysis and Safe Rename workflows
- Rules closing section
- MCP Resources table

- [ ] **Step 5: Commit (if any fixups needed)**

```bash
git add -A
git commit -m "refine(prompts): prompt efficiency overhaul verification"
```

- [ ] **Step 6: Squash or leave as separate commits**

All 5 task commits can stay separate (each is self-contained and tested) or
be squashed into one. User's choice.
