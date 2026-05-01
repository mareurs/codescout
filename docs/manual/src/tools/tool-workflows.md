# Tool Workflows

Named multi-tool chains for common agent tasks. Each workflow is a step-by-step
recipe triggered by a recognizable intent.

---

## Why Workflows?

The decision table in server instructions maps *what you know* to *which tool
to start with*. But it doesn't answer "what's the full sequence?" Workflows
fill that gap — they guide you through multi-step chains where each tool's
output feeds the next.

---

## Editing a Markdown Document

**Intent:** "I need to read and edit parts of a structured markdown file."

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `read_file(path)` | Get heading map — see all sections |
| 2 | `read_file(path, headings=[...])` | Read target sections (one call, multiple sections) |
| 3a | `edit_section(path, heading, action, content)` | Whole-section: replace (body only — heading preserved), insert, remove |
| 3b | `edit_file(path, heading=, old_string, new_string)` | Surgical: string replacement scoped to a section |
| 3c | `edit_file(path, edits=[...])` | Batch: multiple edits across sections, atomic |

**Tips:**
- Start with the heading map (step 1) — don't jump straight to editing.
- Use `headings=[]` (step 2) instead of `mode="complete"` unless you need the entire file.
- Choose step 3a/3b/3c based on scope: whole section → `edit_section`, single fix → `edit_file(heading=)`, multiple fixes → `edit_file(edits=[])`.

---

## Impact Analysis — "What breaks if I change X?"

**Intent:** "I'm about to modify a function/struct/trait and need to understand the blast radius."

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `symbols(name, include_body=true)` | Read the current implementation |
| 2 | `references(name_path, path)` | Find all callers and dependents |
| 3 | `symbol_at` with `fields: ["hover"]` on key call sites | Reveal concrete types flowing through (especially generics/traits) |
| 4 | Edit with full knowledge of impact | |

**Why not `search_pattern`?** A regex search for a symbol name returns string
matches — including imports, type annotations, comments, and tests.
`references` returns only *actual usages* that will break if the API changes.

**Tips:**
- Step 2 may overflow on widely-used symbols. Check the `by_file` distribution to focus on the most important callers.
- Step 3 is optional but valuable for generic code — `symbol_at` (hover) shows the *resolved* concrete type, not the declared generic.

---

## Dependency Tracing — "How does data flow from A to B?"

**Intent:** "I need to trace how a value flows through the call chain — request handling, pipeline stages, error propagation."

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `symbols(entry_point)` | Locate the starting function |
| 2 | `symbol_at` with `fields: ["def"]` on called functions | Follow the call chain forward |
| 3 | `symbol_at` with `fields: ["hover"]` on parameters/return values | See resolved types at each stage |
| 4 | `references` at the destination | Confirm which callers reach this point |

**Why not grep?** `symbol_at` follows the *actual* dispatch — through
trait impls, re-exports, and type aliases. `search_pattern` finds text matches
but can't follow indirection.

**Tips:**
- Use `symbol_at` iteratively — follow the chain function by function.
- The `hover` field at each step shows the concrete types, which is critical when tracing through generics or trait objects.

---

## Safe Rename — "Rename X without breaking anything"

**Intent:** "I need to rename a symbol and verify nothing was missed."

| Step | Tool | Purpose |
|------|------|---------|
| 1 | `references(name_path, path)` | Map all usages before renaming |
| 2 | `rename_symbol(name_path, path, new_name)` | LSP-powered rename across files |
| 3 | `search_pattern(old_name)` | Catch stragglers in comments, strings, docs |
| 4 | `run_command("cargo check")` | Verify compilation |

**Why both `rename_symbol` and `search_pattern`?** LSP rename handles code
references precisely, but it can miss occurrences in string literals, comments,
and documentation. Step 3 catches those stragglers. Step 1 gives you the
expected count to verify against.

**Tips:**
- Compare the count from step 1 with the results from step 3 — any remaining matches after step 2 are the stragglers that need manual attention.
- Always run step 4. `rename_symbol` can occasionally corrupt string literals containing the old name.
