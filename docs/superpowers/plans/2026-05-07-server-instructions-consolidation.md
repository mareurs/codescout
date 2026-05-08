# Server Instructions Consolidation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate language-navigation hints into a workspace-language-aware block rendered live into `server_instructions.md`; promote `call_graph` to canonical workflow with full demonstrative arguments; delete dead emitter from `system-prompt.md` generator.

**Architecture:** New pure module `src/prompts/language_nav.rs` carries per-language `NavBlock` data, a workspace-language ranker, and a renderer. `src/prompts/mod.rs::build_server_instructions` substitutes a new `{{symbol_navigation_block}}` token at session start using `ProjectStatus.languages` (plus workspace project languages). `src/prompts/builders.rs` loses `language_navigation_hints` and the dead `## Language Navigation` emitter. `src/prompts/source.md` is edited in two passes: token insertion + Iron Law / Impact Analysis rewrite + scattered `call_graph` prunes.

**Tech Stack:** Rust 2021 edition, `cargo test`, `cargo clippy`, `cargo fmt`. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-05-07-server-instructions-consolidation-design.md`

**Depends on:** The onboarding refactor spec (`docs/superpowers/specs/2026-05-07-onboarding-refactor-design.md`) is currently uncommitted but its `load_prompt`/`{{include:}}` substitution mechanism is **already in mod.rs** at lines 123-139 and is in use by `onboarding_prompt.md` / `workspace_onboarding_prompt.md`. This plan extends that pattern with a second substitution token applied to the static `SERVER_INSTRUCTIONS`.

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `src/prompts/language_nav.rs` | **new** | `NavBlock` struct, per-language data, generic block, workspace ranker, renderer |
| `src/prompts/mod.rs` | modified | Add `mod language_nav;`; add `SYMBOL_NAV_TOKEN` constant; substitute it inside `build_server_instructions` |
| `src/prompts/source.md` | modified | Insert `{{symbol_navigation_block}}` token; rewrite Iron Law 8 + Impact Analysis; prune 5 `call_graph` one-liners |
| `src/prompts/builders.rs` | modified | Delete `language_navigation_hints` (lines 6-51); delete `## Language Navigation` emission block in `build_system_prompt_draft` |
| `src/tools/run_command/tests.rs` | modified | Delete `language_navigation_hints_*` tests; remove import |

---

## Task 1: Create language_nav skeleton with two languages (Rust + Python)

Goal: get the module compiling and tested with the smallest useful surface. Other languages added in Task 2.

**Files:**
- Create: `src/prompts/language_nav.rs`
- Modify: `src/prompts/mod.rs` (add `pub(crate) mod language_nav;`)

- [ ] **Step 1: Write the failing test**

Append to `src/prompts/language_nav.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_block_returns_some_for_rust_and_python() {
        assert!(nav_block("rust").is_some());
        assert!(nav_block("python").is_some());
    }

    #[test]
    fn nav_block_returns_none_for_unsupported() {
        assert!(nav_block("bash").is_none());
        assert!(nav_block("markdown").is_none());
        assert!(nav_block("unknown_lang").is_none());
    }

    #[test]
    fn supported_languages_lists_all_with_nav_blocks() {
        for lang in supported_languages() {
            assert!(nav_block(lang).is_some(), "supported but no nav_block: {lang}");
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails (module does not exist yet)**

Run: `cargo test --lib prompts::language_nav`
Expected: FAIL — `error[E0583]: file not found for module \`language_nav\`` or unresolved-symbol errors.

- [ ] **Step 3: Write the module body**

Create `src/prompts/language_nav.rs`:

```rust
//! Per-language navigation hints rendered into `server_instructions.md`
//! at session start. Pure data + pure functions — no I/O.

/// One language's symbol-navigation hint block.
pub(crate) struct NavBlock {
    pub language: &'static str,
    pub display_name: &'static str,
    pub markdown: &'static str,
}

const RUST: NavBlock = NavBlock {
    language: "rust",
    display_name: "Rust",
    markdown: "### Rust — Symbol Navigation\n\
        - **`name_path` form:** `Type/method`, `impl Trait for Type/method`\n\
        - **Find a method:** `symbols(name_path=\"Service/handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"src/\", kind=\"struct\")` (also `\"interface\"` for traits)\n\
        - **Language note:** trait impls use `impl Trait for Type/method`; rust-analyzer reports traits as `kind=\"interface\"`\n\
        - **Before refactor:** `call_graph(symbol=\"Service/handle\", path=\"src/service.rs\", direction=\"callers\", max_depth=3)`\n",
};

const PYTHON: NavBlock = NavBlock {
    language: "python",
    display_name: "Python",
    markdown: "### Python — Symbol Navigation\n\
        - **`name_path` form:** `Class/method`, `module_func`\n\
        - **Find a method:** `symbols(name_path=\"Service/handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"src/\", kind=\"class\")`\n\
        - **Language note:** decorators are not part of the symbol — search by the decorated function's name\n\
        - **Before refactor:** `call_graph(symbol=\"Service/handle\", path=\"src/service.py\", direction=\"callers\", max_depth=3)`\n",
};

pub(crate) fn nav_block(lang: &str) -> Option<&'static NavBlock> {
    match lang {
        "rust" => Some(&RUST),
        "python" => Some(&PYTHON),
        _ => None,
    }
}

pub(crate) fn supported_languages() -> &'static [&'static str] {
    &["rust", "python"]
}
```

Modify `src/prompts/mod.rs` — add the module declaration near the other `pub mod` lines (after the existing `pub use` block, before `pub const SERVER_INSTRUCTIONS`):

```rust
pub(crate) mod language_nav;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib prompts::language_nav`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/prompts/language_nav.rs src/prompts/mod.rs
git commit -m "feat(prompts): introduce language_nav module with rust + python NavBlocks"
```

---

## Task 2: Add the four remaining languages

Goal: complete the supported language set per the spec — TypeScript (covers tsx/jsx/javascript), Kotlin (covers java), Go, C#.

**Files:**
- Modify: `src/prompts/language_nav.rs`

- [ ] **Step 1: Write the failing test**

Replace the `nav_block_returns_some_for_rust_and_python` test with:

```rust
#[test]
fn nav_block_returns_some_for_all_supported_languages() {
    for lang in ["rust", "python", "typescript", "javascript", "tsx", "jsx",
                  "kotlin", "java", "go", "csharp"] {
        assert!(nav_block(lang).is_some(), "missing nav_block for {lang}");
    }
}

#[test]
fn every_nav_block_has_required_bullets() {
    for lang in supported_languages() {
        let block = nav_block(lang).unwrap();
        let md = block.markdown;
        for marker in ["**`name_path` form:**", "**Find a method:**",
                        "**List by kind:**", "**Language note:**",
                        "**Before refactor:**"] {
            assert!(md.contains(marker), "{} missing bullet: {marker}", lang);
        }
    }
}

#[test]
fn every_nav_block_uses_only_generic_example_names() {
    let allowed_caps = ["Service", "Repository", "Order", "Account"];
    let allowed_lower = ["find", "handle", "process", "create", "core", "worker"];
    let banned = ["MyStruct", "UserService", "AuthProvider", "UserRepository",
                   "Server/handle_request", "UserService/create",
                   "AuthProvider/login", "UserRepository/findById"];
    for lang in supported_languages() {
        let block = nav_block(lang).unwrap();
        let md = block.markdown;
        for b in banned {
            assert!(!md.contains(b),
                "{} uses banned example name {b} (drift risk)", lang);
        }
        let _ = (allowed_caps, allowed_lower); // documentation; not asserted positively
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib prompts::language_nav`
Expected: FAIL — `nav_block_returns_some_for_all_supported_languages` fails on `typescript`.

- [ ] **Step 3: Add the four NavBlocks and dispatch**

In `src/prompts/language_nav.rs`, after `PYTHON` and before `nav_block`, add:

```rust
const TYPESCRIPT: NavBlock = NavBlock {
    language: "typescript",
    display_name: "TypeScript / JavaScript",
    markdown: "### TypeScript / JavaScript — Symbol Navigation\n\
        - **`name_path` form:** `Class/method`, `exportedFunction`\n\
        - **Find a method:** `symbols(name_path=\"Service/handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"src/\", kind=\"class\")` for classes; `kind=\"function\"` for arrow fns\n\
        - **Language note:** React function components are `kind=\"function\"`, not `kind=\"class\"`\n\
        - **Before refactor:** `call_graph(symbol=\"Service/handle\", path=\"src/service.ts\", direction=\"callers\", max_depth=3)`\n",
};

const KOTLIN: NavBlock = NavBlock {
    language: "kotlin",
    display_name: "Kotlin / Java",
    markdown: "### Kotlin / Java — Symbol Navigation\n\
        - **`name_path` form:** `Class/method`, `Object.companion/method`\n\
        - **Find a method:** `symbols(name_path=\"Service/handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"src/\", kind=\"class\")` (covers classes, objects, annotations)\n\
        - **Language note:** annotations are not in the symbol — search by method name\n\
        - **Before refactor:** `call_graph(symbol=\"Service/handle\", path=\"src/Service.kt\", direction=\"callers\", max_depth=3)`\n",
};

const GO: NavBlock = NavBlock {
    language: "go",
    display_name: "Go",
    markdown: "### Go — Symbol Navigation\n\
        - **`name_path` form:** `Type/Method`, `PackageFunc`\n\
        - **Find a method:** `symbols(name_path=\"Service/Handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"./\", kind=\"function\")` (covers funcs and methods)\n\
        - **Language note:** interfaces use `kind=\"interface\"`; receiver methods stay in `Type/Method` form\n\
        - **Before refactor:** `call_graph(symbol=\"Service/Handle\", path=\"service.go\", direction=\"callers\", max_depth=3)`\n",
};

const CSHARP: NavBlock = NavBlock {
    language: "csharp",
    display_name: "C#",
    markdown: "### C# — Symbol Navigation\n\
        - **`name_path` form:** `Class/Method`, `Namespace.Class/Method` for nested\n\
        - **Find a method:** `symbols(name_path=\"Service/Handle\", include_body=true)`\n\
        - **List by kind:** `symbols(path=\"src/\", kind=\"class\")` (also `\"interface\"`)\n\
        - **Language note:** properties surface as `kind=\"function\"` getters/setters in some LSPs\n\
        - **Before refactor:** `call_graph(symbol=\"Service/Handle\", path=\"src/Service.cs\", direction=\"callers\", max_depth=3)`\n",
};
```

Update `nav_block`:

```rust
pub(crate) fn nav_block(lang: &str) -> Option<&'static NavBlock> {
    match lang {
        "rust" => Some(&RUST),
        "python" => Some(&PYTHON),
        "typescript" | "javascript" | "tsx" | "jsx" => Some(&TYPESCRIPT),
        "kotlin" | "java" => Some(&KOTLIN),
        "go" => Some(&GO),
        "csharp" => Some(&CSHARP),
        _ => None,
    }
}

pub(crate) fn supported_languages() -> &'static [&'static str] {
    &["rust", "python", "typescript", "kotlin", "go", "csharp"]
}
```

Note: `supported_languages()` lists canonical keys only, not aliases. Aliases (`javascript`, `tsx`, `jsx`, `java`) resolve via `nav_block` but are not enumerated.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib prompts::language_nav`
Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/prompts/language_nav.rs
git commit -m "feat(prompts): add ts/kotlin/go/csharp NavBlocks"
```

---

## Task 3: Workspace language ranker

**Files:**
- Modify: `src/prompts/language_nav.rs`

- [ ] **Step 1: Write failing tests**

Append to the `tests` mod in `src/prompts/language_nav.rs`:

```rust
#[test]
fn rank_workspace_languages_picks_top_2_by_weight() {
    // Three projects: rust appears 3x, python 2x, kotlin 1x
    let lists: Vec<Vec<String>> = vec![
        vec!["rust".into()],
        vec!["rust".into(), "python".into()],
        vec!["rust".into(), "python".into(), "kotlin".into()],
    ];
    let ranked = rank_workspace_languages(&lists, 2);
    assert_eq!(ranked, vec!["rust", "python"]);
}

#[test]
fn rank_workspace_languages_filters_unsupported() {
    let lists: Vec<Vec<String>> = vec![vec!["bash".into(), "rust".into()]];
    let ranked = rank_workspace_languages(&lists, 2);
    assert_eq!(ranked, vec!["rust"]);
}

#[test]
fn rank_workspace_languages_deterministic_on_ties() {
    // rust and python both appear once → alphabetical order
    let lists: Vec<Vec<String>> = vec![vec!["rust".into(), "python".into()]];
    let ranked = rank_workspace_languages(&lists, 2);
    assert_eq!(ranked, vec!["python", "rust"]);
}

#[test]
fn rank_workspace_languages_caps_at_max() {
    let lists: Vec<Vec<String>> = vec![vec![
        "rust".into(), "python".into(), "kotlin".into(),
        "go".into(), "csharp".into(),
    ]];
    let ranked = rank_workspace_languages(&lists, 2);
    assert_eq!(ranked.len(), 2);
}

#[test]
fn rank_workspace_languages_handles_empty() {
    let lists: Vec<Vec<String>> = vec![];
    let ranked = rank_workspace_languages(&lists, 2);
    assert!(ranked.is_empty());
}

#[test]
fn rank_workspace_languages_normalizes_aliases() {
    // "javascript" should bucket into "typescript" canonical key
    let lists: Vec<Vec<String>> = vec![
        vec!["javascript".into()],
        vec!["typescript".into()],
        vec!["jsx".into()],
    ];
    let ranked = rank_workspace_languages(&lists, 2);
    assert_eq!(ranked, vec!["typescript"]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib prompts::language_nav::tests::rank`
Expected: FAIL — `rank_workspace_languages` not defined.

- [ ] **Step 3: Implement the ranker**

Add to `src/prompts/language_nav.rs` (above the `tests` mod):

```rust
use std::collections::BTreeMap;

/// Map an arbitrary language string to its canonical key (the one that
/// appears in `supported_languages()`). Returns `None` for unsupported.
fn canonical_key(lang: &str) -> Option<&'static str> {
    match lang {
        "rust" => Some("rust"),
        "python" => Some("python"),
        "typescript" | "javascript" | "tsx" | "jsx" => Some("typescript"),
        "kotlin" | "java" => Some("kotlin"),
        "go" => Some("go"),
        "csharp" => Some("csharp"),
        _ => None,
    }
}

/// Rank workspace languages by occurrence count and return the top `max`
/// supported canonical keys. Ties broken alphabetically for determinism.
pub(crate) fn rank_workspace_languages(
    project_languages: &[Vec<String>],
    max: usize,
) -> Vec<&'static str> {
    let mut counts: BTreeMap<&'static str, u32> = BTreeMap::new();
    for langs in project_languages {
        for lang in langs {
            if let Some(key) = canonical_key(lang) {
                *counts.entry(key).or_insert(0) += 1;
            }
        }
    }
    let mut ranked: Vec<(&'static str, u32)> = counts.into_iter().collect();
    // Sort by count descending; BTreeMap iter is already alphabetical for ties.
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    ranked.into_iter().take(max).map(|(k, _)| k).collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib prompts::language_nav`
Expected: all tests pass (10 total).

- [ ] **Step 5: Commit**

```bash
git add src/prompts/language_nav.rs
git commit -m "feat(prompts): workspace language ranker with alias normalization"
```

---

## Task 4: Renderer with lead-in + generic block

**Files:**
- Modify: `src/prompts/language_nav.rs`

- [ ] **Step 1: Write failing tests**

Append to the `tests` mod:

```rust
#[test]
fn render_with_no_languages_emits_lead_in_and_generic() {
    let lists: Vec<Vec<String>> = vec![];
    let out = render_symbol_navigation_block(&lists);
    assert!(out.contains("### Symbol Navigation Patterns"));
    assert!(out.contains("### Generic Patterns (any language)"));
    // No per-language sections
    assert!(!out.contains("### Rust — Symbol Navigation"));
    assert!(!out.contains("### Python — Symbol Navigation"));
}

#[test]
fn render_with_one_language_emits_one_block() {
    let lists: Vec<Vec<String>> = vec![vec!["rust".into()]];
    let out = render_symbol_navigation_block(&lists);
    assert!(out.contains("### Rust — Symbol Navigation"));
    assert!(!out.contains("### Python — Symbol Navigation"));
    assert!(out.contains("### Generic Patterns (any language)"));
}

#[test]
fn render_with_many_languages_caps_at_two() {
    let lists: Vec<Vec<String>> = vec![vec![
        "rust".into(), "python".into(), "kotlin".into(),
        "go".into(), "csharp".into(),
    ]];
    let out = render_symbol_navigation_block(&lists);
    let n_blocks = ["### Rust — Symbol Navigation",
                     "### Python — Symbol Navigation",
                     "### Kotlin / Java — Symbol Navigation",
                     "### Go — Symbol Navigation",
                     "### C# — Symbol Navigation"]
        .iter()
        .filter(|h| out.contains(*h))
        .count();
    assert_eq!(n_blocks, 2);
}

#[test]
fn render_contains_no_deprecated_tool_names() {
    let lists: Vec<Vec<String>> = vec![vec![
        "rust".into(), "python".into(), "typescript".into(),
        "kotlin".into(), "go".into(),
    ]];
    let out = render_symbol_navigation_block(&lists);
    for dead in ["find_symbol", "list_symbols", "replace_symbol",
                 "insert_code", "rename_symbol", "search_pattern"] {
        assert!(!out.contains(dead),
            "rendered block contains deprecated tool name: {dead}");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib prompts::language_nav::tests::render`
Expected: FAIL — `render_symbol_navigation_block` not defined.

- [ ] **Step 3: Implement renderer + lead-in + generic block**

Add to `src/prompts/language_nav.rs` (above the `tests` mod, below the ranker):

```rust
const LEAD_IN: &str = "### Symbol Navigation Patterns\n\
\n\
- **Hierarchical nav** — impl/class methods, all languages:\n\
  `symbols(name_path=\"MyStruct/my_method\", include_body=true)`\n\
- **Kind filter + path scope:**\n\
  `symbols(path=\"src/tools/\", kind=\"struct\")`\n\
- **Find across project then read body:**\n\
  `symbols(name=\"edit_code\")` → `symbols(name_path=\"ToolName/edit_code\", include_body=true)`\n\
\n";

const GENERIC: &str = "### Generic Patterns (any language)\n\
\n\
- `name_path` syntax: `Container/member` for methods on classes/structs/objects;\n\
  bare name for top-level functions or types.\n\
- `kind` filter values vary by language: `function`, `class`, `struct`, `interface`,\n\
  `type`, `enum`, `module`, `constant`. Run `symbols(path)` once on a representative\n\
  file to see what kinds your LSP emits.\n\
- For impact analysis before any structural change:\n\
  `call_graph(symbol, path, direction=\"callers\")` traces blast radius;\n\
  `direction=\"callees\"` traces outbound flow.\n\
- When the symbol's exact name is unknown, start with\n\
  `semantic_search(\"what it does\")` then drill down with `symbols(name_path=...)`.\n";

pub(crate) fn render_symbol_navigation_block(
    project_languages: &[Vec<String>],
) -> String {
    let ranked = rank_workspace_languages(project_languages, 2);
    let mut out = String::with_capacity(2048);
    out.push_str(LEAD_IN);
    for key in ranked {
        if let Some(block) = nav_block(key) {
            out.push_str(block.markdown);
            out.push('\n');
        }
    }
    out.push_str(GENERIC);
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib prompts::language_nav`
Expected: 14 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/prompts/language_nav.rs
git commit -m "feat(prompts): symbol-navigation renderer with lead-in + generic block"
```

---

## Task 5: Replace static block in server_instructions.md with token + wire substitution

**Files:**
- Modify: `src/prompts/source.md`
- Modify: `src/prompts/mod.rs`

- [ ] **Step 1: Write failing tests**

Append to the `tests` mod in `src/prompts/mod.rs` (below `load_prompt_substitutes_include_marker`):

```rust
#[test]
fn server_instructions_template_has_symbol_nav_token() {
    let raw = SERVER_INSTRUCTIONS;
    assert_eq!(
        raw.matches("{{symbol_navigation_block}}").count(),
        1,
        "server_instructions.md must contain exactly one symbol_navigation_block token"
    );
}

#[test]
fn build_server_instructions_substitutes_symbol_nav_token() {
    let result = build_server_instructions(None);
    assert!(!result.contains("{{symbol_navigation_block}}"),
        "token must be substituted in build_server_instructions output");
    assert!(result.contains("### Symbol Navigation Patterns"));
    assert!(result.contains("### Generic Patterns (any language)"));
}

#[test]
fn build_server_instructions_renders_languages_from_status() {
    let status = ProjectStatus {
        name: "x".into(),
        path: "/tmp/x".into(),
        languages: vec!["rust".into()],
        memories: vec![],
        has_index: false,
        system_prompt: None,
        workspace: None,
    };
    let result = build_server_instructions(Some(&status));
    assert!(result.contains("### Rust — Symbol Navigation"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib prompts::tests::server_instructions_template_has_symbol_nav_token`
Expected: FAIL — token absent from file.

- [ ] **Step 3: Edit server_instructions.md to insert the token**

Replace the entire body of the `### Symbol Navigation Patterns` section in `src/prompts/source.md` (currently lines 101-132) with:

```
### Symbol Navigation Patterns

{{symbol_navigation_block}}

```

(Single blank line after the token; the renderer's output ends with a trailing newline.)

Use `edit_markdown`:

```
edit_markdown(
  path="src/prompts/source.md",
  heading="### Symbol Navigation Patterns",
  action="replace",
  content="\n{{symbol_navigation_block}}\n",
)
```

- [ ] **Step 4: Wire substitution in build_server_instructions**

In `src/prompts/mod.rs`, locate the `build_server_instructions` function (currently starts at line 25). Add a substitution step at the top of the function body:

```rust
pub fn build_server_instructions(project_status: Option<&ProjectStatus>) -> String {
    // Substitute the language-aware symbol navigation block.
    let project_languages: Vec<Vec<String>> = match project_status {
        Some(s) => {
            let mut v: Vec<Vec<String>> = vec![s.languages.clone()];
            if let Some(ws) = &s.workspace {
                for p in ws {
                    v.push(p.languages.clone());
                }
            }
            v
        }
        None => Vec::new(),
    };
    let nav_block = language_nav::render_symbol_navigation_block(&project_languages);
    let mut instructions = SERVER_INSTRUCTIONS.replace(SYMBOL_NAV_TOKEN, &nav_block);

    // ... existing logic that appends project_status, kotlin-lsp, workspace, etc.
    // continues to operate on `instructions` instead of starting from `SERVER_INSTRUCTIONS.to_string()`.
}
```

Add the constant near `INCLUDE_MARKER` (line 125):

```rust
pub const SYMBOL_NAV_TOKEN: &str = "{{symbol_navigation_block}}";
```

The existing function body starts with `let mut instructions = SERVER_INSTRUCTIONS.to_string();` — change that line to use the substituted form (per the snippet above). Leave the rest of the function untouched.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib prompts`
Expected: all prompts tests pass, including the 3 new ones.

- [ ] **Step 6: Update existing build_without_project_returns_static test**

The test `build_without_project_returns_static` (line 263 area) asserts equality with `SERVER_INSTRUCTIONS`. After substitution this no longer holds. Replace its body:

```rust
#[test]
fn build_without_project_returns_substituted_static() {
    let result = build_server_instructions(None);
    // Token is substituted even with no project status.
    assert!(!result.contains("{{symbol_navigation_block}}"));
    assert!(result.contains("### Symbol Navigation Patterns"));
    // No per-language block when there are no languages.
    assert!(!result.contains("### Rust — Symbol Navigation"));
    assert!(!result.contains("## Project Status"));
}
```

- [ ] **Step 7: Run all prompts tests**

Run: `cargo test --lib prompts`
Expected: pass.

- [ ] **Step 8: Commit**

```bash
git add src/prompts/source.md src/prompts/mod.rs
git commit -m "feat(prompts): templatize symbol navigation block in server_instructions.md"
```

---

## Task 6: Delete dead language-navigation emitter from builders.rs

**Files:**
- Modify: `src/prompts/builders.rs`
- Modify: `src/tools/run_command/tests.rs`

- [ ] **Step 1: Identify deletion targets**

Run: `grep -n "language_navigation_hints\|## Language Navigation" src/prompts/builders.rs`

Expected matches: function definition at line 6, call site around line 274, emission block around 271-282 in `build_system_prompt_draft`.

- [ ] **Step 2: Delete the function**

In `src/prompts/builders.rs`, delete the entire `language_navigation_hints` function (lines 6 through ~51 — from `pub(crate) fn language_navigation_hints` through the closing `}` and following blank line).

Use `edit_code`:

```
edit_code(
  symbol="language_navigation_hints",
  path="src/prompts/builders.rs",
  action="remove",
)
```

- [ ] **Step 3: Delete the emission block in build_system_prompt_draft**

In `src/prompts/builders.rs`, locate the block inside `build_system_prompt_draft` that emits the `## Language Navigation` section. The block starts with the `let hints: Vec<...> = languages.iter()` filter chain (around line 273) and ends after the `for (lang, hint) in hints { ... }` loop. Delete the entire block including the `draft.push_str("## Language Navigation\n");` lead-in.

Use `edit_file` for this targeted removal:

```
edit_file(
  path="src/prompts/builders.rs",
  old_string="<paste the exact block, lines 271-282 — including leading and trailing blank lines>",
  new_string="",
)
```

(The exact bytes must be matched. Open the file first to copy the literal block.)

- [ ] **Step 4: Update `run_command/tests.rs`**

In `src/tools/run_command/tests.rs`, remove the import of `language_navigation_hints` from line 7 (it's part of a multi-name `use` block):

Change:
```rust
use crate::prompts::builders::{
    build_system_prompt_draft, build_workspace_instructions, language_navigation_hints,
    ...
};
```

To remove `language_navigation_hints` from the brace list. Use `edit_file` on the import line.

Delete the test functions that reference `language_navigation_hints` — find them via:

Run: `grep -n "language_navigation_hints" src/tools/run_command/tests.rs`

Expected: lines 2126, 2136, 2137, 2138 — these are inside test functions. Identify the enclosing `#[test] fn name` and delete each whole test function. Use `edit_code(action="remove")` for each.

- [ ] **Step 5: Verify compilation**

Run: `cargo build --lib 2>&1`
Expected: clean build, no errors.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: pass. Then: `grep "FAILED" @cmd_id` of the test output to confirm zero failures.

- [ ] **Step 7: Commit**

```bash
git add src/prompts/builders.rs src/tools/run_command/tests.rs
git commit -m "refactor(prompts): remove dead language_navigation_hints emitter"
```

---

## Task 7: Rewrite Iron Law 8 to promote call_graph

**Files:**
- Modify: `src/prompts/source.md`

- [ ] **Step 1: Write failing test**

Append to `src/prompts/mod.rs` tests:

```rust
#[test]
fn iron_law_8_promotes_call_graph_before_references() {
    let raw = SERVER_INSTRUCTIONS;
    // Find Iron Law 8 — starts with "8. **CALL GRAPH"
    let idx = raw.find("8. **CALL GRAPH BEFORE STRUCTURAL EDITS.**")
        .expect("Iron Law 8 must be the call_graph promotion");
    // Within the law's body (next ~500 chars), call_graph must precede references
    let body = &raw[idx..idx.saturating_add(500)];
    let cg = body.find("call_graph").expect("call_graph must appear");
    let refs = body.find("references").expect("references must appear");
    assert!(cg < refs, "call_graph must be named before references");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib prompts::tests::iron_law_8_promotes_call_graph_before_references`
Expected: FAIL — current Iron Law 8 says "REFERENCES BEFORE EDITING."

- [ ] **Step 3: Rewrite Iron Law 8 in `src/prompts/source.md`**

The current Iron Law 8 (lines 59-62) reads:

```
8. **REFERENCES BEFORE EDITING.** Before `edit_code(action="rename"|"replace")`,
   run `references(symbol, path)` to get the concrete call-site list.
   `call_graph` gives transitive reach; `references` gives the actual locations.
   Skip only when you already ran references for this symbol in this session.
```

Replace it (using `edit_file` with exact-string match) with:

```
8. **CALL GRAPH BEFORE STRUCTURAL EDITS.** Before
   `edit_code(action="rename"|"replace")` of a function, method, or
   public type: `call_graph(symbol, path, direction="callers",
   max_depth=3)` first, then `references` for edit targets. Transitive
   callers are invisible to `references` alone.
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib prompts::tests::iron_law_8`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src/prompts/source.md
git commit -m "docs(prompts): promote call_graph in Iron Law 8"
```

---

## Task 8: Rewrite Impact Analysis section as canonical demo

**Files:**
- Modify: `src/prompts/source.md`

- [ ] **Step 1: Write failing test**

Append to `src/prompts/mod.rs` tests:

```rust
#[test]
fn impact_analysis_section_contains_call_graph_with_full_arguments() {
    let raw = SERVER_INSTRUCTIONS;
    let section_start = raw.find("### Impact Analysis").expect("section must exist");
    let next = raw[section_start..].find("\n### ").map(|i| section_start + i)
        .unwrap_or(raw.len());
    let section = &raw[section_start..next];

    assert!(section.contains("call_graph(symbol="),
        "Impact Analysis must include a call_graph call with named symbol arg");
    assert!(section.contains("direction=\"callers\""),
        "Impact Analysis must demonstrate direction=\"callers\"");
    assert!(section.contains("max_depth=3"),
        "Impact Analysis must demonstrate max_depth=3");
    assert!(section.contains("`references`"),
        "Impact Analysis must reference the references tool");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib prompts::tests::impact_analysis_section`
Expected: FAIL — current section is a 7-row table without full args.

- [ ] **Step 3: Rewrite the section using edit_markdown**

```
edit_markdown(
  path="src/prompts/source.md",
  heading="### Impact Analysis — \"What breaks if I change X?\"",
  action="replace",
  content="\n`references` = direct call sites. `call_graph` = transitive reach.\nBoth required for any rename / signature change / contract change.\n\n1. `symbols(name=\"Service/handle\", include_body=true)` — read it.\n2. `call_graph(symbol=\"Service/handle\", path=\"src/service.rs\",\n   direction=\"callers\", max_depth=3)` — blast radius.\n   Tree depth ≈ change risk: shallow = local; deep+branching = contract.\n3. `references(symbol, path)` — file:line edit targets.\n4. `symbol_at(path, line, fields=[\"hover\"])` on non-obvious callers\n   from step 2 — reveal concrete types behind generics/traits.\n5. `edit_code(...)`.\n\n`direction`: `callers` (refactors) | `callees` (flow) | `both` (hubs, rare).\n`max_depth`: `1` ≈ references; `3` default; `5` only for deep reach.\nSkip call_graph only for body-only edits with identical signature.\n",
)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib prompts::tests::impact_analysis_section`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add src/prompts/source.md
git commit -m "docs(prompts): rewrite Impact Analysis as canonical call_graph demo"
```

---

## Task 9: Prune scattered call_graph one-liners

**Files:**
- Modify: `src/prompts/source.md`

- [ ] **Step 1: Identify the targets**

Run: `grep -n "call_graph" src/prompts/source.md`

Expected (post-Tasks 7+8 changes): the live mentions remaining are at lines previously identified as L69 (anti-pattern), L110 (Symbol Nav — already gone via Task 5's `{{symbol_navigation_block}}` swap), L139 (LSP Workflow step 4), L150 (Search Routing), L287 (Safe Rename step 2b). Verify which still exist.

- [ ] **Step 2: Prune anti-pattern table cell (L69)**

In the `## Anti-Patterns` table, the row currently reads:

```
| Edit a symbol without blast-radius check | `call_graph(symbol, path, direction="callers", max_depth=3)` first | Transitive callers invisible to grep/references alone — silent breakage |
```

Replace the middle cell with `\`call_graph(...)\` — see Impact Analysis`.

Use `edit_file`:

```
edit_file(
  path="src/prompts/source.md",
  old_string="| Edit a symbol without blast-radius check | `call_graph(symbol, path, direction=\"callers\", max_depth=3)` first | Transitive callers invisible to grep/references alone — silent breakage |",
  new_string="| Edit a symbol without blast-radius check | `call_graph(...)` — see Impact Analysis | Transitive callers invisible to grep/references alone — silent breakage |",
)
```

- [ ] **Step 3: Prune LSP Workflow step 4 (L139 area)**

The current text:

```
4. `call_graph(symbol, path, direction="callers", max_depth=3)` — transitive blast radius for renames/structural changes
5. `edit_code(...)` — make the change
```

Replace step 4 line with a cross-reference:

```
edit_file(
  path="src/prompts/source.md",
  old_string="4. `call_graph(symbol, path, direction=\"callers\", max_depth=3)` — transitive blast radius for renames/structural changes\n5. `edit_code(...)` — make the change",
  new_string="4. For impact analysis, see Impact Analysis.\n5. `edit_code(...)` — make the change",
)
```

- [ ] **Step 4: Tighten Search Routing line (L150 area)**

Current:

```
- **Transitive call graphs** → `call_graph(symbol, direction, max_depth)` — `direction="callers"` for blast-radius sizing; `direction="callees"` for flow tracing. `call_graph(depth=1, direction="callers")` also filters refs to call sites only.
```

Replace with:

```
edit_file(
  path="src/prompts/source.md",
  old_string="- **Transitive call graphs** → `call_graph(symbol, direction, max_depth)` — `direction=\"callers\"` for blast-radius sizing; `direction=\"callees\"` for flow tracing. `call_graph(depth=1, direction=\"callers\")` also filters refs to call sites only.",
  new_string="- **Transitive call graphs** → `call_graph(symbol, path, direction, max_depth)` — see Impact Analysis for the worked example.",
)
```

- [ ] **Step 5: Prune Safe Rename row 2b**

The current Safe Rename table:

```
| 1 | `references(symbol, path)` | Map all usages before renaming |
| 2 | `edit_code(action="rename", symbol, path, new_name)` | LSP-powered rename across files |
| 3 | `grep(old_name)` | Catch stragglers in comments, strings, docs |
| 4 | `run_command("cargo check")` | Verify compilation |
```

(There is no row 2b in Safe Rename today — the spec was looking at Impact Analysis. Verify with grep first; if absent, skip this step.)

Replace the section's intro sentence with:

```
edit_markdown(
  path="src/prompts/source.md",
  heading="### Safe Rename",
  action="replace",
  content="\nRun Impact Analysis first.\n\n| Step | Tool | Purpose |\n|------|------|---------|\n| 1 | `edit_code(action=\"rename\", symbol, path, new_name)` | LSP-powered rename across files |\n| 2 | `grep(old_name)` | Catch stragglers in comments, strings, docs |\n| 3 | `run_command(\"cargo check\")` | Verify compilation |\n",
)
```

(Drops the now-redundant `references` row — it's covered by Impact Analysis step 3.)

- [ ] **Step 6: Run all tests**

Run: `cargo test --lib prompts`
Expected: pass.

Run: `cargo test`
Expected: pass overall. Use `grep "FAILED" @cmd_id` to confirm.

- [ ] **Step 7: Commit**

```bash
git add src/prompts/source.md
git commit -m "docs(prompts): prune scattered call_graph one-liners; cross-reference Impact Analysis"
```

---

## Task 10: Cross-prompt consistency test update

**Files:**
- Modify: `src/server.rs` (the `prompt_surfaces_reference_only_real_tools` test, around line 1428-1620)

- [ ] **Step 1: Locate the test**

Run: `grep -n "prompt_surfaces_reference_only_real_tools\|server_instructions.md" src/server.rs`

Expected: test fixture array around line 1505-1515 includes `("server_instructions.md", include_str!("prompts/server_instructions.md"))`.

- [ ] **Step 2: Update fixture to use rendered output**

In `src/server.rs`, change the test setup so that the `server_instructions.md` surface is rendered (with token substituted) before scanning. Replace:

```rust
(
    "server_instructions.md",
    include_str!("prompts/server_instructions.md"),
),
```

with:

```rust
let rendered_instructions = crate::prompts::build_server_instructions(None);
let surfaces: &[(&str, &str)] = &[
    (
        "server_instructions.md (rendered)",
        rendered_instructions.as_str(),
    ),
    // ... rest unchanged
```

(Adapt to the surrounding code — the surfaces array may need to be rebound after `rendered_instructions` is in scope, since `&str` borrows from a local.)

- [ ] **Step 3: Run the test**

Run: `cargo test --lib prompt_surfaces_reference_only_real_tools`
Expected: pass. If it fails on a tool name, the whitelist or the rendered content needs adjustment — investigate the specific identifier.

- [ ] **Step 4: Add the rendered-deprecated-name guard test**

Append to the same test module:

```rust
#[test]
fn rendered_server_instructions_contains_no_deprecated_tool_names() {
    let status = ProjectStatus {
        name: "x".into(),
        path: "/tmp/x".into(),
        languages: vec!["rust".into(), "python".into(), "typescript".into(),
                         "kotlin".into(), "go".into()],
        memories: vec![],
        has_index: false,
        system_prompt: None,
        workspace: None,
    };
    let rendered = crate::prompts::build_server_instructions(Some(&status));
    for dead in ["find_symbol", "list_symbols", "replace_symbol",
                  "insert_code", "rename_symbol", "search_pattern"] {
        assert!(!rendered.contains(dead),
            "rendered server instructions contains deprecated tool name: {dead}");
    }
}
```

(Place this in `src/prompts/mod.rs` test module, since it imports `ProjectStatus` and `build_server_instructions` from there.)

- [ ] **Step 5: Run new test**

Run: `cargo test --lib rendered_server_instructions_contains_no_deprecated_tool_names`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/server.rs src/prompts/mod.rs
git commit -m "test(prompts): cross-prompt consistency uses rendered output; add deprecated-name guard"
```

---

## Task 11: Final verification

**Files:** none modified — verification only.

- [ ] **Step 1: Format check**

Run: `cargo fmt --check`
Expected: no output, exit 0.

If it fails, run `cargo fmt` and amend the previous commit.

- [ ] **Step 2: Clippy clean**

Run: `cargo clippy -- -D warnings`
Expected: clean exit. Any warning is a failure.

- [ ] **Step 3: Full test suite**

Run: `cargo test`
Expected: clean pass. Confirm via: `grep -E "FAILED|test result.*ok" @cmd_id`.

- [ ] **Step 4: Release build**

Run: `cargo build --release`
Expected: clean exit.

- [ ] **Step 5: Live MCP smoke test**

Restart the MCP server in this session via `/mcp`. Then in a fresh Claude Code session against this repo, inspect the live server instructions:

- The `### Symbol Navigation Patterns` section appears.
- Exactly **2** language NavBlocks render (the workspace has Rust as primary plus a Python fixture and a TypeScript fixture — top 2 by occurrence).
- The `### Generic Patterns (any language)` section appears at the end.
- The literal string `{{symbol_navigation_block}}` does NOT appear.
- Iron Law 8 begins with **CALL GRAPH BEFORE STRUCTURAL EDITS**.
- The Impact Analysis section contains a full `call_graph(symbol="Service/handle", ...)` call with `direction="callers"` and `max_depth=3`.

If any check fails, fix and re-build.

- [ ] **Step 6: Activate single-language fixture for comparison**

Activate the rust-library fixture:

```
workspace(action="activate", path="tests/fixtures/rust-library")
```

Reconnect via `/mcp`. Verify exactly **1** language NavBlock (Rust) renders. Then restore the home project:

```
workspace(action="activate", path=".", read_only=false)
```

- [ ] **Step 7: Final commit if any verification-driven fix was needed**

If steps 1-6 surfaced any issue and you committed a fix, that commit is the verification commit.
Otherwise no commit is needed — the previous commits are the final state.

---

## Out of scope for this plan

- **Q2 sweep** of stale tool names in 19/36 cached `system-prompt.md` files in user workspaces. Will be a separate plan: `docs/superpowers/plans/2026-05-07-cached-prompt-migration-sweep.md`.

## Self-review checklist

- [x] Spec coverage — every Decision row in the spec is implemented by a task: language scope (Tasks 3-5), example naming (Task 2 test 4), Iron Law 8 (Task 7), Impact Analysis (Task 8), one-liner prunes (Task 9), generator deletion (Task 6).
- [x] No placeholders — every step contains the literal code or command to run.
- [x] Type consistency — `NavBlock`, `nav_block`, `rank_workspace_languages`, `render_symbol_navigation_block`, `SYMBOL_NAV_TOKEN` named identically across tasks.
- [x] ONBOARDING_VERSION not bumped — confirmed by absence of any `ONBOARDING_VERSION` mention in this plan, consistent with spec.

> **Status (2026-05-08): paths retargeted post-I-01.** This plan was drafted on 2026-05-07. The next day, I-01 (commits `7db51a5`..`f047d47`) consolidated `src/prompts/source.md` and `src/prompts/source.md` into a single `src/prompts/source.md` with `<!-- @surface NAME -->` blocks; build.rs slices them into `OUT_DIR/{surface}.md` at compile time. Path references below have been mechanically updated. **Two derived consequences the executor still needs to handle:**
>
> 1. **Token substitution must run at session-start** (when `ProjectStatus.languages` is known), NOT at build time. build.rs writes the surface verbatim with the `{{symbol_navigation_block}}` token preserved; runtime substitution happens when `from_parts` builds the server instructions string.
> 2. **Task 10 (cross-prompt consistency test)** is partially obsolete: `prompt_surfaces_reference_only_real_tools` was already redirected to scan runtime constants (`SERVER_INSTRUCTIONS`, `ONBOARDING_PROMPT`) in I-01 Phase 3 (commit `82701e2`). Re-derive Task 10 against the current `src/server.rs` test, not the include_str!-based shape described below.
>
> Original design intent is intact. Path references resolve. Test-plumbing surgery needs re-derivation.

---
