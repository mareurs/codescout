//! Prompt construction helpers used by onboarding, workspace, and
//! prompt-refresh flows. Pure string-building — no tool side effects.

use std::path::Path;

/// Returns curated anti-patterns and correct patterns for a language.
/// Content sourced from docs/research/claude-language-patterns.md.
pub(crate) fn language_patterns(lang: &str) -> Option<&'static str> {
    match lang {
        "rust" => Some(
            "### Rust\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. Gratuitous `.clone()` to silence borrow checker → borrow: `&str` over `&String`, `&[T]` over `&Vec<T>`\n\
             2. `.unwrap()` everywhere → `?` with `.context()` from anyhow, `.expect(\"invariant: ...\")` only for proven invariants\n\
             3. `Rc<RefCell<T>>` / interior mutability overuse → restructure data flow and ownership\n\
             4. `String` params where `&str` suffices → `fn greet(name: &str)`, use `Cow<'_, str>` when ownership is conditional\n\
             5. Catch-all `_ => {}` in match → handle all variants explicitly, let compiler check exhaustiveness\n\
             \n\
             **Correct patterns:**\n\
             1. `thiserror` for library errors, `anyhow` for application errors — propagate with `?`\n\
             2. Iterator chains over explicit loops — `.iter().map(f).collect()`, avoid unnecessary `.collect()`\n\
             3. `Vec::with_capacity()` when size is known\n\
             4. Derive common traits: `#[derive(Debug, Clone, PartialEq)]`, `#[derive(Default)]` when sensible\n\
             5. `if let`/`while let` for single-pattern matching instead of full match",
        ),
        "python" => Some(
            "### Python\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. Mutable default arguments `def f(items=[])` → use `None` with `if items is None: items = []`\n\
             2. `typing.List`, `typing.Dict`, `typing.Optional` → built-in generics: `list[str]`, `str | None`\n\
             3. Bare/broad exception handling `except Exception: pass` → catch specific exceptions, log with context\n\
             4. `os.path.join()` → `pathlib.Path`: `Path(base) / \"data\" / \"file.csv\"`\n\
             5. `Any` type overuse → complete type annotations on all function signatures\n\
             \n\
             **Correct patterns:**\n\
             1. Modern type hints (3.10+): `list[int]`, `dict[str, Any]`, `str | None`\n\
             2. `uv` for packages, `ruff` for linting/formatting, `pyright` for types, `pytest` for testing\n\
             3. `pyproject.toml` over `setup.py`/`requirements.txt`\n\
             4. `dataclasses` for internal data, Pydantic for validation, TypedDict for dict shapes\n\
             5. `is` comparison for singletons: `if x is None:` not `if x == None:`",
        ),
        "typescript" => Some(
            "### TypeScript\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. `any` type overuse → `unknown` when type is uncertain, Zod schemas for external data\n\
             2. Type assertion `as` abuse / `as unknown as T` → type guards, proper narrowing\n\
             3. Missing discriminated unions → model domain states with `'kind'`/`'type'` discriminant, `satisfies never` for exhaustiveness\n\
             4. Non-null assertion `!` abuse → handle null/undefined with narrowing, optional chaining, type guards\n\
             5. Enums → `as const` objects or string literal union types\n\
             \n\
             **Correct patterns:**\n\
             1. Strict tsconfig: `strict: true`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`\n\
             2. Explicit return types on exported functions\n\
             3. Zod schema validation for external data — derive types with `z.infer<typeof Schema>`\n\
             4. Discriminated unions with exhaustiveness: `default: throw new Error(\\`Unhandled: ${x satisfies never}\\`)`\n\
             5. `interface` for object shapes, `type` for unions/intersections/mapped types",
        ),
        "javascript" | "jsx" => Some(
            "### JavaScript\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. Missing Promise error handling → every `.then()` needs `.catch()`, every `async/await` needs try/catch\n\
             2. Stale closures in React hooks → ensure exhaustive dependency arrays in useEffect/useCallback/useMemo\n\
             3. Event listener / timer memory leaks → cleanup with `removeEventListener`, `clearInterval`, `AbortController`\n\
             4. `var` declarations → `const` by default, `let` only for reassignment\n\
             5. Loose equality `==` → always `===` and `!==`\n\
             \n\
             **Correct patterns:**\n\
             1. Proper useEffect async: define async inside effect, call it, return cleanup with AbortController\n\
             2. `const` by default, destructuring at function boundaries\n\
             3. Named exports over default exports — aids tree-shaking and refactoring\n\
             4. Template literals over string concatenation\n\
             5. `jsconfig.json` with `checkJs: true` for type safety in JS projects",
        ),
        "go" => Some(
            "### Go\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. `ioutil` package → `io.ReadAll`, `os.ReadFile`, `os.MkdirTemp` (deprecated since Go 1.16)\n\
             2. Pre-modern patterns → `slices.Contains()`, `min`/`max` builtins (1.21), `for range n` (1.22)\n\
             3. Java-style large interfaces at producer → accept interfaces at consumer, return structs, keep interfaces small (1-3 methods)\n\
             4. Error wrapping with `%v` → `fmt.Errorf(\"context: %w\", err)`, use `errors.Is`/`errors.As`\n\
             5. `context.Background()` deep in call chains → ctx as first param, pass through entire chain, never store in structs\n\
             \n\
             **Correct patterns:**\n\
             1. Table-driven tests with `t.Parallel()` and `t.Run()` subtests\n\
             2. `errgroup` for structured concurrency: `g, ctx := errgroup.WithContext(ctx)`\n\
             3. Functional options pattern: `WithPort(8080)`, `WithTimeout(30*time.Second)`\n\
             4. `slog` for structured logging (Go 1.21+), not `log.Println`\n\
             5. No name stuttering: `package kv; type Store` not `type KVStore`",
        ),
        "java" => Some(
            "### Java\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. `@Autowired` field injection → constructor injection with `final` fields (Spring 4.3+ auto-infers)\n\
             2. `Optional.get()` without check → `orElseThrow(() -> new NotFoundException(id))`, Optional for return types only\n\
             3. `throws Exception` / bare catches → declare and catch specific exceptions, log with context\n\
             4. `Date`/`Calendar`/`SimpleDateFormat` → `java.time`: `LocalDate`, `ZonedDateTime`, `DateTimeFormatter`\n\
             5. Raw types `List items` → `List<String> items = new ArrayList<>()`\n\
             \n\
             **Correct patterns:**\n\
             1. Records for data carriers (Java 16+): `public record UserDto(String name, String email) {}`\n\
             2. Sealed classes + pattern matching (Java 17+/21+) with switch expressions\n\
             3. Text blocks `\"\"\"` for multi-line strings (Java 15+)\n\
             4. Pattern matching instanceof (Java 16+): `if (obj instanceof String s) { s.length(); }`\n\
             5. Immutable collections: `List.of()`, `Map.of()`, `Set.of()`",
        ),
        "kotlin" => Some(
            "### Kotlin\n\
             \n\
             **Anti-patterns (Don't → Do):**\n\
             1. `!!` (not-null assertion) overuse → `?.let`, `?:`, `?.` chaining, or redesign to eliminate nullability\n\
             2. `GlobalScope.launch`/`async` → lifecycle-bound scopes: `viewModelScope`, `lifecycleScope`, injected `CoroutineScope`\n\
             3. `runBlocking` in production code → only for `main()` and tests, use suspend functions\n\
             4. Mutable `var` in data classes → `val` + `List` (not `MutableList`), immutability by default\n\
             5. `enum` when sealed class is needed → `sealed class`/`sealed interface` for state with per-variant data\n\
             \n\
             **Correct patterns:**\n\
             1. `val` over `var`, `List` over `MutableList` — expose read-only interfaces\n\
             2. Structured concurrency: `coroutineScope { launch { a() }; launch { b() } }`\n\
             3. Sealed class/interface for all state and result types\n\
             4. `Sequence` for large collections with chained operations\n\
             5. `require`/`check`/`error` for preconditions: `require(age >= 0) { \"Age must be non-negative\" }`",
        ),
        _ => None,
    }
}

/// Assembles a language-patterns memory from detected project languages.
/// Returns None if no detected languages have pattern data.
pub(crate) fn build_language_patterns_memory(languages: &[String]) -> Option<String> {
    let sections: Vec<&str> = languages
        .iter()
        .filter_map(|lang| language_patterns(lang))
        .collect();

    if sections.is_empty() {
        return None;
    }

    let mut content = String::from(
        "# Language Patterns\n\n\
         Per-language anti-patterns and correct patterns for this project's languages.\n\
         Each section lists the top 5 mistakes LLMs make and the top 5 idiomatic patterns.\n\n",
    );

    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            content.push_str("\n---\n\n");
        }
        content.push_str(section);
        content.push('\n');
    }

    Some(content)
}

pub(crate) fn build_system_prompt_draft(
    languages: &[String],
    entry_points: &[String],
    project_root: Option<&Path>,
    projects: Option<&[crate::workspace::DiscoveredProject]>,
    libraries: &[crate::library::registry::LibraryEntry],
) -> String {
    let mut draft = String::new();
    draft.push_str("# Project — Code Explorer Guidance\n\n");

    let projects_slice = projects.unwrap_or(&[]);

    // Entry points section
    draft.push_str("## Entry Points\n");
    if entry_points.is_empty() {
        draft.push_str("- Explore with `tree(\".\")` then `symbols` on key files\n");
    } else {
        for ep in entry_points {
            draft.push_str(&format!("- `{}` — start here\n", ep));
        }
    }
    draft.push('\n');

    // Key abstractions — placeholder for the LLM to fill
    draft.push_str("## Key Abstractions\n");
    draft.push_str("- [3-5 entries max. Each = one line: `TypeName` (`path/`) — one-line purpose only. No narrative.]\n\n");

    // Search tips
    draft.push_str("## Search Tips\n");
    if !languages.is_empty() {
        draft.push_str(&format!("- This is a {} project\n", languages.join("/")));
    }
    draft.push_str("- Use specific terms over generic ones (e.g., avoid 'data', 'utils')\n");
    draft.push_str("- For call relationships and impact analysis: `call_graph(symbol, path)` — traces callers/callees\n");
    if projects_slice.len() > 1 {
        draft.push_str(
            "- **Workspace mode:** always scope `semantic_search` with `project_id=\"<id>\"` — \
             broad terms match all projects and return mixed results\n",
        );
        for p in projects_slice {
            let example_term = if p.languages.iter().any(|l| l == "rust") {
                "key type or trait name"
            } else if p
                .languages
                .iter()
                .any(|l| l == "typescript" || l == "javascript")
            {
                "handler or component name"
            } else if p.languages.iter().any(|l| l == "python") {
                "class or function name"
            } else {
                "concept specific to this project"
            };
            draft.push_str(&format!(
                "  - `{}`: `semantic_search(\"<{}>\", project_id=\"{}\")` \
                 — [fill in good query examples during onboarding]\n",
                p.id, example_term, p.id
            ));
        }
    }
    draft.push('\n');

    // Language patterns reference — only if at least one language has patterns
    let has_patterns = languages.iter().any(|l| language_patterns(l).is_some());
    if has_patterns {
        let pattern_langs: Vec<&str> = languages
            .iter()
            .filter(|l| language_patterns(l).is_some())
            .map(|s| s.as_str())
            .collect();
        draft.push_str("## Language Patterns\n");
        draft.push_str(&format!(
            "This project uses {}. Read `memory(action=\"read\", topic=\"language-patterns\")` before writing, editing, or reviewing code.\n\n",
            pattern_langs.join(", ")
        ));
    }

    // Navigation strategy — per-project subsections for multi-project workspaces
    if projects_slice.len() > 1 {
        draft.push_str("## Navigation Strategy\n\n");
        draft.push_str("1. `memory(action=\"read\", topic=\"architecture\")` — orient yourself to the workspace\n");
        draft.push_str(
            "2. `semantic_search(\"your concept\")` — find relevant code across projects\n",
        );
        draft.push_str(
            "3. `memory(action=\"recall\", query=\"...\")` — search memories by meaning\n\n",
        );
        draft.push_str("**Per-project navigation:**\n\n");
        for p in projects_slice {
            let langs = if p.languages.is_empty() {
                String::new()
            } else {
                format!(" ({})", p.languages.join(", "))
            };
            draft.push_str(&format!("### {}{}\n", p.id, langs));
            draft.push_str(&format!(
                "1. `symbols(\"{}\")` — [fill in entry point during onboarding]\n",
                p.relative_root.display()
            ));
            draft.push_str(&format!(
                "2. `semantic_search(\"your concept\", project_id=\"{}\")` — search within this project\n",
                p.id
            ));
            draft.push_str(&format!(
                "3. `memory(project_id=\"{}\", action=\"read\", topic=\"architecture\")` — project-specific knowledge\n",
                p.id
            ));
            draft.push_str(
                "3b. `symbol_at(path, line)` — hover + type sig when you have an exact location\n",
            );
            draft.push_str("3c. `references(symbol, path)` — all call sites before any edit\n");
            draft.push_str(
                "4. `call_graph(symbol=\"Name\", path=\"...\", direction=\"callers\")` — blast radius before any structural change; `direction=\"callees\"` for flow tracing\n\n",
            );
        }

        draft.push_str("**Cross-project navigation:**\n");
        draft.push_str("- **Quick lookups** (1–3 calls): pass `project_id=\"<id>\"` to scope the call — no state change.\n");
        draft.push_str("- **Sustained exploration** (reading memories, semantic search, many tool calls): \
                         use `workspace(action=\"activate\", path=\"<id>\")`, but **always `workspace(action=\"activate\")` back to your original \
                         project when done.** Forgetting to return leaves all subsequent tool calls operating \
                         against the wrong project.\n");
        draft.push_str("- **Subagents:** the MCP server state is shared with the parent conversation. \
                         You **MUST** `workspace(action=\"activate\")` back to the original project before completing your task.\n\n");
        draft.push_str(
            "**Markdown files** (memories, plans, docs): \
             `read_markdown(\"path\")` — returns heading map + `@file_ref` for large files. \
             **IRON LAW #6:** subsequent reads MUST use `@file_ref` (not the original path): \
             `read_markdown(\"@file_ref\", heading=\"## Section\")` or `start_line=/end_line=`.\n\n",
        );
    } else {
        draft.push_str("## Navigation Strategy\n");
        draft.push_str("1. `memory(action=\"read\", topic=\"architecture\")` — orient yourself\n");
        if !entry_points.is_empty() {
            draft.push_str(&format!(
                "2. `symbols(\"{}\")` — see main structure\n",
                entry_points[0]
            ));
        } else {
            draft.push_str("2. `symbols(\"src/\")` — see main structure\n");
        }
        draft.push_str("3. `semantic_search(\"your concept\")` — find relevant code\n");
        draft.push_str("4. `symbols(name=\"Name\", include_body=true)` — read implementation\n");
        draft.push_str("   - regex-like patterns belong in `grep`, not `symbols`\n");
        draft.push_str("4b. `symbol_at(path, line)` — hover + type sig when you have an exact location from prior tool output; skip re-searching\n");
        draft.push_str("4c. `references(symbol, path)` — all call sites before any edit\n");
        draft.push_str(
            "5. `call_graph(symbol=\"Name\", direction=\"callers\")` — transitive blast radius; `direction=\"callees\"` for flow tracing\n",
        );
        draft.push_str(
            "6. `memory(action=\"recall\", query=\"...\")` — search memories by meaning\n\n",
        );
        draft.push_str(
            "7. `read_markdown(\"path/to/file.md\")` — returns heading map + `@file_ref` for large files. \
             **IRON LAW #6:** subsequent reads MUST use `@file_ref` (not the original path): \
             `read_markdown(\"@file_ref\", heading=\"## Section\")` or `start_line=/end_line=`.\n\n",
        );
    }

    // Retrieval stack — semantic_search routes through Qdrant
    draft.push_str("## Retrieval Stack\n");
    draft.push_str(
        "`semantic_search` runs through the Qdrant + TEI hybrid stack. \
         Start it once per machine with `./scripts/retrieval-stack.sh up`, then index this \
         project with `cargo run --release --bin sync_project -- <path> <project_id>`. \
         If a call returns `retrieval stack offline`, the stack isn't running.\n\n",
    );

    // MCP resource pointers — always included so agents know where to get extended docs
    draft.push_str("## MCP Resources\n");
    draft.push_str(
        "Extended docs and project context are available via MCP resources (`resources/read <uri>`):\n",
    );
    draft.push_str("- `doc://codescout-tool-guide` — long-form usage notes for every tool (examples, tradeoffs)\n");
    draft.push_str(
        "- `memory://<name>` — project memory files (architecture, conventions, gotchas)\n",
    );
    draft.push_str("- `project://summary` — active project + index + LSP snapshot\n\n");

    // Project rules — empty section for the LLM to fill from exploration
    draft.push_str("## Project Rules\n");
    draft.push_str("- [Fill from Phase 1 exploration: linting, formatting, commit conventions]\n");

    // Workspace projects table — only for multi-project repos
    if projects_slice.len() > 1 {
        draft.push_str("\n## Workspace Projects\n\n");
        draft.push_str("| Project | Root | Languages | Build |\n");
        draft.push_str("|---------|------|-----------|-------|\n");
        for p in projects_slice {
            draft.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                p.id,
                p.relative_root.display(),
                p.languages.join(", "),
                p.manifest.as_deref().unwrap_or("-"),
            ));
        }
        draft.push('\n');

        // Load depends_on from workspace.toml if available
        if let Some(root) = project_root {
            let ws_path = crate::config::workspace::workspace_config_path(root);
            if let Ok(content) = std::fs::read_to_string(&ws_path) {
                if let Ok(ws) =
                    toml::from_str::<crate::config::workspace::WorkspaceConfig>(&content)
                {
                    let deps: Vec<_> = ws
                        .projects
                        .iter()
                        .filter(|p| !p.depends_on.is_empty())
                        .collect();
                    if !deps.is_empty() {
                        draft.push_str("**Cross-project dependencies:**\n");
                        for p in deps {
                            draft.push_str(&format!(
                                "- {} depends on {}\n",
                                p.id,
                                p.depends_on.join(", "),
                            ));
                        }
                        draft.push('\n');
                    }
                }
            }
        }

        draft.push_str(
            "Use `project_id=\"name\"` parameter to scope search/navigation to a specific project.\n\n",
        );

        draft.push_str(
            "**Per-project details:** Use `memory(project_id=\"<id>\", topic=\"architecture\")` \
             or `memory(project_id=\"<id>\", topic=\"conventions\")` for project-specific knowledge.\n\n",
        );
    }

    // Registered libraries — only included when at least one is registered
    if !libraries.is_empty() {
        draft.push_str("\n## Registered Libraries\n\n");
        for lib in libraries {
            let status = if lib.indexed {
                if lib.version.is_some()
                    && lib.version_indexed.is_some()
                    && lib.version != lib.version_indexed
                {
                    "indexed [stale]"
                } else {
                    "indexed"
                }
            } else {
                "not indexed"
            };
            draft.push_str(&format!(
                "- **{}** ({}) — {}\n",
                lib.name, lib.language, status
            ));
        }
        draft.push_str(
            "\nUse `scope=\"lib:<name>\"` with `symbols`, `grep`, \
             and `semantic_search` to navigate library code. \
             Run `index(action=\"build\", scope=\"lib:<name>\")` to enable semantic search for a library.\n",
        );
    }

    // Preferences auto-inject moved out of this sync builder — see
    // `append_preferences_section()` and its caller in onboarding.rs.
    let _ = project_root;

    draft
}
/// Append a `## User Preferences` section to a system-prompt draft using the
/// top-10 most-recently-updated memories in the `preferences` bucket.
///
/// Lives outside `build_system_prompt_draft` because the storage layer is
/// async (Qdrant) and the draft builder must stay sync to keep its 16+ test
/// call sites simple. Best-effort: any failure (store unavailable, empty,
/// network error) leaves `draft` untouched.
pub(crate) async fn append_preferences_section(agent: &crate::agent::Agent, draft: &mut String) {
    let project_id = {
        let inner = agent.inner.read().await;
        match inner.active_project() {
            Some(p) => p.config.project.name.clone(),
            None => return,
        }
    };
    let store = match agent.semantic_memory_store().await {
        Ok(s) => s,
        Err(_) => return,
    };
    let filter = crate::memory::semantic_store::MemoryFilter {
        bucket: Some("preferences".into()),
        order_by: crate::memory::semantic_store::MemoryOrder::UpdatedAtDesc,
        limit: Some(10),
        ..Default::default()
    };
    let hits = match store.list(&project_id, filter).await {
        Ok(h) => h,
        Err(_) => return,
    };
    if hits.is_empty() {
        return;
    }
    draft.push_str("\n## User Preferences\n\n");
    for hit in &hits {
        let m = &hit.memory;
        let summary = if m.content.len() > 200 {
            let end = crate::tools::floor_char_boundary(&m.content, 200);
            format!("{}...", &m.content[..end])
        } else {
            m.content.clone()
        };
        draft.push_str(&format!("- **{}:** {}\n", m.title, summary));
    }
}

/// Build the preamble prepended to the onboarding prompt for the subagent.
/// Instructs the subagent to activate the project before following the exploration steps.
pub(crate) fn build_subagent_preamble() -> String {
    let mut s = String::new();
    s.push_str("You are an onboarding subagent for codescout. ");
    s.push_str("Your job is to thoroughly explore this codebase and write project memories ");
    s.push_str("that will be used by every future session.\n\n");
    s.push_str("FIRST ACTION: Call workspace(action=\"activate\", path=\".\", read_only=false) to initialize the ");
    s.push_str("project context. All subsequent tool calls depend on this.\n\n");
    s.push_str("Then follow the exploration and memory-writing instructions below exactly.\n\n");
    s.push_str("---\n\n");
    s
}

/// Build the epilogue appended to the onboarding prompt for the subagent.
/// Defines the return contract: what the subagent must include in its final response.
pub(crate) fn build_subagent_epilogue() -> String {
    let mut s = String::new();
    s.push_str("\n---\n\n");
    s.push_str("## Return Contract\n\n");
    s.push_str(
        "When you have completed ALL exploration steps and written ALL memories, end your \
response with this structured summary:\n\n",
    );
    s.push_str("**Exploration Summary:**\n");
    s.push_str("- What this system does (your own words, not the README's)\n");
    s.push_str("- The 5 most important types/modules (name, file, role)\n");
    s.push_str("- How a typical operation flows (concrete function names)\n");
    s.push_str("- What surprised you (things docs didn't mention)\n\n");
    s.push_str("**Memories Written:**\n");
    s.push_str(
        "- List each memory topic you wrote (e.g., \"architecture\", \"conventions\", etc.)\n\n",
    );
    s.push_str("**Warnings:**\n");
    s.push_str(
        "- Any issues encountered (index not built, LSP failures, files that couldn't be read)\n",
    );
    s.push_str("- Steps you couldn't fully complete and why\n\n");
    s.push_str(
        "This summary is returned to the main agent and shown to the user. Make it \
informative but concise — aim for 300-500 tokens total.\n\n",
    );
    s.push_str(
        "LAST ACTION: Call workspace(action=\"activate\", path=\".\") before returning to ensure the parent's \
project state is unchanged.",
    );
    s
}

/// Extract level-2 headings from markdown content with numbered line counts.
pub(crate) fn build_heading_map(prompt: &str) -> Vec<String> {
    let lines: Vec<&str> = prompt.lines().collect();
    let mut headings: Vec<(String, usize)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("## ") {
            headings.push((line.to_string(), i));
        }
    }
    headings
        .iter()
        .enumerate()
        .map(|(idx, (heading, start))| {
            let end = headings
                .get(idx + 1)
                .map(|(_, s)| *s)
                .unwrap_or(lines.len());
            format!("{}. {} ({} lines)", idx + 1, heading, end - start)
        })
        .collect()
}

/// Build dispatch instructions for full onboarding, adapted to client capability.
pub(crate) fn build_buffered_onboarding_instructions(
    prompt_path: &str,
    subagent_capable: bool,
) -> String {
    let p = prompt_path;
    let checklist = format!(
        "\
Read these phases in order — complete each before moving to the next:

  1. read_markdown(\"{p}\", heading=\"## THE IRON LAW\")
  2. read_markdown(\"{p}\", headings=[\"## Phase 0: Embedding Model Selection\", \"## Phase 1: Semantic Index Check\"])
  3. read_markdown(\"{p}\", heading=\"## Phase 2: Explore the Code\")
     → Execute ALL exploration steps before continuing
  4. read_markdown(\"{p}\", headings=[\"## Red Flags — STOP and Return to Phase 2\", \"## Common Rationalizations\"])
  5. read_markdown(\"{p}\", heading=\"## Phase 3: Write the Memories (Single-Project Mode)\")
     → Write all memories before continuing
  6. read_markdown(\"{p}\", headings=[\"## After Everything Is Created\", \"## Gathered Project Data\", \"## Return Contract\"])

For workspace mode, read these after step 2 (before step 3):
  read_markdown(\"{p}\", headings=[\"## Workspace Survey — Breadth-First\", \"## Workspace Deep Dives — Subagent Per-Project\"])
And after step 4 (before step 5):
  read_markdown(\"{p}\", heading=\"## Workspace Memory Synthesis\")"
    );

    if subagent_capable {
        format!(
            "\
Onboarding required — this project has not been explored yet.

Spawn a general-purpose subagent with model=sonnet to perform the exploration and \
memory writing. The subagent must read the onboarding prompt by phase — \
copy-paste these exact commands:

{checklist}

Do NOT read the onboarding prompt yourself — let the subagent handle it.

When the subagent completes, report its summary to the user. Then read whichever \
memories are relevant to the user's current task via memory(action=\"read\", topic=...).

Wait for the subagent to complete before continuing — onboarding is a prerequisite \
for all subsequent work.

If you cannot spawn subagents, follow the checklist yourself."
        )
    } else {
        format!(
            "\
Onboarding required — this project has not been explored yet.

{checklist}

Follow each phase's instructions to explore the codebase and write project memories."
        )
    }
}

/// Build dispatch instructions for version refresh, adapted to client capability.
pub(crate) fn build_buffered_refresh_instructions(
    prompt_path: &str,
    stored: Option<u32>,
    current: u32,
    subagent_capable: bool,
) -> String {
    let stored_str = stored
        .map(|v| format!("v{v}"))
        .unwrap_or_else(|| "pre-versioning".to_string());

    if subagent_capable {
        format!(
            "\
System prompt outdated ({stored_str} → v{current}) — a lightweight refresh is needed.

Spawn a general-purpose subagent with model=sonnet to regenerate the system prompt. \
The subagent must:
  read_markdown(\"{prompt_path}\")  — read the full refresh prompt (it's short)

The subagent will re-read memories and regenerate system-prompt.md without \
re-exploring the codebase.

When the subagent completes, continue with the user's original task."
        )
    } else {
        format!(
            "\
System prompt outdated ({stored_str} → v{current}) — a lightweight refresh is needed.

Read the refresh prompt:
  read_markdown(\"{prompt_path}\")

Follow it to re-read memories and regenerate system-prompt.md."
        )
    }
}

/// Build the subagent task prompt for refreshing the system prompt.
///
/// The subagent will re-read memories and regenerate system-prompt.md without
/// re-exploring the codebase — memory already captured all the relevant knowledge.
pub(crate) fn build_prompt_refresh_subagent_prompt(memory_topics: &[String]) -> String {
    let memory_reads = memory_topics
        .iter()
        .filter(|t| t.as_str() != "system-prompt")
        .map(|t| format!("  - memory(action=\"read\", topic=\"{t}\")"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "\
System prompt refresh — the stored onboarding version is behind the current codescout version.

Steps:
1. workspace(action=\"activate\", path=\".\", read_only=false) — enable writes
2. Read each project memory that contributes to the system prompt:
{memory_reads}
3. Read the current system-prompt.md (if it exists) for Entry Points structure only — do NOT copy tool navigation examples from it, as those may be stale
4. Regenerate system-prompt.md following the canonical template spec:

   **What to include:**
   - Entry points: specific file paths + symbol names to start exploring
   - Key abstractions: **3-5 entries max**. Each = one line: `TypeName` (`path/`) — one-line purpose only. NO architecture narrative, NO state machine descriptions, NO config details — those go in the `architecture` memory.
   - Search tips: concrete query examples that work well for THIS codebase; terms to avoid
   - Navigation strategy: recommended tool call sequence for a new task. Every step must name a codescout tool. Include `call_graph(symbol, path, direction=\"callers\")` for blast-radius checking before edits, and `direction=\"callees\"` for tracing data/control flow.
   - Project rules: conventions the AI must follow that linters don't catch

   **What NOT to include:**
   - How codescout tools work (static tool guidance covers this)
   - Full architecture details (architecture memory covers this)
   - Command lists, glossary, detailed conventions (other memories cover these)
   - More than ~30 lines total (injected every session — keep it dense)
   - Native host tool names (Read, Grep, Glob, Edit, Bash) — blocked in codescout sessions

   **Template:**
   ```
   # [Project Name] — Code Explorer Guidance

   ## Entry Points
   [Specific files + symbols, not module descriptions]

   ## Key Abstractions
   [3-5 lines: `TypeName` (`path/`) — one-line purpose]

   ## Search Tips
   [Concrete queries + terms to avoid]

   ## Navigation Strategy
   [Numbered steps, each naming a codescout tool. Include call_graph step.]

   ## Project Rules
   [Conventions not caught by linters]
   ```

5. Write the updated content to .codescout/system-prompt.md
6. Do NOT re-explore the codebase — the memories already contain the relevant knowledge
7. workspace(action=\"activate\", path=\".\") — restore normal state

When done, report: \"System prompt refreshed (vN → vM).\"",
        memory_reads = memory_reads,
    )
}

/// Build a self-contained onboarding prompt for one project in a workspace.
///
/// Each per-project subagent gets this prompt. It includes:
/// - Iron Law, exploration steps, red flags (from the shared template)
/// - Project-specific context (id, root, languages, siblings)
/// - Per-project memory writing instructions (scoped with project= param)
/// - Return contract
#[allow(dead_code)]
pub(crate) fn build_per_project_prompt(
    project: &crate::workspace::DiscoveredProject,
    siblings: &[(String, Vec<String>)],
) -> String {
    let mut prompt = String::new();

    // Iron Law (from shared template)
    prompt.push_str("## THE IRON LAW\n\n");
    prompt.push_str(
        "```\nNO MEMORIES WRITTEN WITHOUT COMPLETING ALL EXPLORATION STEPS FIRST\n```\n\n",
    );
    prompt.push_str("You may only call `memory(action: \"write\", ...)` after you have:\n");
    prompt.push_str("1. Completed ALL exploration steps below\n");
    prompt.push_str("2. Verified EVERY item in the Phase 2 Gate Checklist\n\n");
    prompt.push_str("---\n\n");

    // Project context
    prompt.push_str("## Your Project\n\n");
    prompt.push_str(&format!("- **ID:** {}\n", project.id));
    prompt.push_str(&format!(
        "- **Root:** {}\n",
        project.relative_root.display()
    ));
    prompt.push_str(&format!(
        "- **Languages:** {}\n",
        project.languages.join(", ")
    ));
    if let Some(ref manifest) = project.manifest {
        prompt.push_str(&format!("- **Manifest:** {}\n", manifest));
    }

    if !siblings.is_empty() {
        prompt.push_str("\n**Sibling projects** (for context — Do NOT deep-dive these):\n");
        for (id, langs) in siblings {
            prompt.push_str(&format!("- {} ({})\n", id, langs.join(", ")));
        }
    }
    prompt.push_str("\n---\n\n");

    // Phase 2: Explore (scoped to this project)
    prompt.push_str("## Phase 2: Explore the Code\n\n");
    prompt.push_str("Explore ONLY your project root. Do NOT explore sibling projects.\n\n");
    prompt.push_str(&format!(
        "### Step 1: Map the Codebase Structure\n\n\
         - `tree(\"{root}\")` — top-level structure\n\
         - `tree` on each subdirectory\n\
         - `read_file` on the build config\n\
         - `read_markdown(\"README.md\")` if present\n\n",
        root = project.relative_root.display()
    ));
    prompt.push_str(
        "### Step 2: Full Symbol Survey\n\n\
         - Run `symbols` on the main source directory\n\
         - Run `symbols` on EACH subdirectory individually\n\
         - Survey at least 5 distinct source files\n\n",
    );
    prompt.push_str(
        "### Step 3: Read Core Implementations\n\n\
         - Identify 5+ central types/functions from Step 2\n\
         - `symbols(name=..., include_body=true)` for each\n\
         - Read the FULL body, not just signatures\n\n",
    );
    prompt.push_str(
        "### Step 4: Read Architecture Documentation\n\n\
         - `read_markdown` on any docs found in the project\n\
         - Read completely — do not skim\n\n",
    );
    prompt.push_str(
        "### Step 5: Trace Two Data Flows\n\n\
         - Trace the most representative operation end-to-end\n\
         - Trace a second distinct path (error, write vs read, etc.)\n\n",
    );
    prompt.push_str(
        "### Step 6: Concept-Level Search (5+ queries)\n\n\
         - Error handling, data flow, testing, config, domain concept\n\
         - Use `semantic_search` or `grep` as fallback\n\n",
    );
    prompt.push_str(
        "### Step 7: Examine Tests\n\n\
         - `symbols` on test directory\n\
         - Read 2-3 test files for patterns\n\n",
    );

    // Phase 2 Gate Checklist
    prompt.push_str(
        "### Phase 2 Gate Checklist\n\n\
         Before writing ANY memory, verify ALL true:\n\
         - [ ] Listed structure AND ran tree on major subdirectories\n\
         - [ ] Symbol survey on 5+ source files\n\
         - [ ] Read full body of 5+ core implementations\n\
         - [ ] Read all architecture docs\n\
         - [ ] Traced two data flows\n\
         - [ ] Ran 5+ concept queries\n\
         - [ ] Read 2-3 test files\n\n\
         ---\n\n",
    );

    // Red Flags
    prompt.push_str(
        "## Red Flags — STOP and Return to Phase 2\n\n\
         If you notice any of these, STOP and go back:\n\
         - \"I have a good enough picture\" — No, read the code.\n\
         - \"The README covers this\" — READMEs lie. Verify in code.\n\
         - \"This is similar to...\" — Explore anyway. Differences matter.\n\n\
         ---\n\n",
    );

    // Phase 3: Write per-project memories
    prompt.push_str("## Phase 3: Write the Memories\n\n");
    prompt.push_str(&format!(
        "Write these memories using `memory(action=\"write\", project=\"{id}\", topic=\"...\", content=\"...\")`.\n\n",
        id = project.id
    ));
    prompt.push_str(
        "### 1. `project-overview`\n\
         Purpose, tech stack, key dependencies, runtime requirements. 15-30 lines.\n\n\
         ### 2. `architecture`\n\
         Module structure, key abstractions, data flow, design patterns. 20-40 lines.\n\
         Include 3-5 good `semantic_search(query, project=\"{id}\")` examples.\n\n\
         ### 3. `conventions`\n\
         Language/framework patterns, naming, testing approach. 15-30 lines.\n\n",
    );

    // Return contract
    prompt.push_str("---\n\n");
    prompt.push_str(
        "## Return Contract\n\n\
         Return a summary with:\n\
         - What this project does (your own words)\n\
         - 3-5 most important types/modules\n\
         - How a typical operation flows\n\
         - Memories written (list topics)\n\
         - Any issues encountered\n",
    );

    prompt
}

/// Build the workspace synthesis prompt that runs after all per-project subagents complete.
///
/// The main agent (or a synthesis subagent) reads this to:
/// 1. Read back all per-project memories
/// 2. Write 5 workspace-level memories
/// 3. Generate the system prompt
/// 4. Offer to refresh CLAUDE.md with memory references
#[allow(dead_code)]
pub(crate) fn build_synthesis_prompt(projects: &[(String, Vec<String>)]) -> String {
    let mut prompt = String::new();

    // Step 1: Read back per-project memories
    prompt.push_str("## Read Per-Project Memories\n\n");
    prompt.push_str("Read these memories to understand what each subagent discovered:\n\n");
    for (id, _langs) in projects {
        prompt.push_str(&format!(
            "- `memory(action=\"read\", project=\"{id}\", topic=\"project-overview\")`\n\
             - `memory(action=\"read\", project=\"{id}\", topic=\"architecture\")`\n\
             - `memory(action=\"read\", project=\"{id}\", topic=\"conventions\")`\n"
        ));
    }
    prompt.push_str("\n---\n\n");

    // Step 2: Write workspace-level memories
    prompt.push_str("## Write Workspace Memories\n\n");
    prompt.push_str(
        "Write these 5 workspace-level memories (no `project:` parameter = workspace-level):\n\n",
    );
    prompt.push_str(
        "### 1. `architecture`\n\
         Workspace-level architecture:\n\
         - Project map: each project's purpose (1 sentence each)\n\
         - Cross-project dependencies (which imports from which)\n\
         - Shared infrastructure (CI, deployment, tooling)\n\
         15-30 lines.\n\n\
         ### 2. `conventions`\n\
         Shared patterns across projects: commit style, PR process, CI rules.\n\
         Per-project: reference `memory(project=\"{id}\", topic=\"conventions\")`.\n\
         15-30 lines.\n\n\
         ### 3. `development-commands`\n\
         Workspace-level build/test/lint commands. Per-project commands go in per-project memories.\n\
         10-20 lines.\n\n\
         ### 4. `domain-glossary`\n\
         Terms used across multiple projects. Project-specific terms go in per-project memories.\n\
         10-20 lines.\n\n\
         ### 5. `gotchas`\n\
         Cross-project pitfalls, version mismatches, integration gotchas.\n\
         10-20 lines.\n\n",
    );

    // Step 3: System prompt
    prompt.push_str("---\n\n## Generate System Prompt\n\n");
    prompt.push_str(
        "Write `system-prompt.md` using `memory(action=\"write\", topic=\"system-prompt\", content=\"...\")`.\n\
         Include: entry points per project, key abstractions, search tips scoped by project,\n\
         navigation strategy for the workspace.\n\n",
    );

    // Step 4: CLAUDE.md refresh
    prompt.push_str("---\n\n## Refresh CLAUDE.md\n\n");
    prompt.push_str(
        "Read `read_markdown(\"CLAUDE.md\")` to see its heading structure.\n\n\
         Compare each section with the memories you just wrote. For sections that\n\
         overlap with memory content, offer to replace the body with a memory reference:\n\
         `See codescout memory 'architecture' (Key Patterns section).`\n\n\
         **preserve user-specific content:** personal preferences, code style rules,\n\
         iron rules, git workflow specifics, private notes — anything not derivable\n\
         from the codebase. Do NOT touch sections the user wrote for their own use.\n\n\
         **Add memory discovery hints** if CLAUDE.md doesn't already list available memories.\n\n\
         Present a summary of proposed changes and ask for approval before modifying.\n\n",
    );

    // Return contract
    prompt.push_str("---\n\n## Return Contract\n\n");
    prompt.push_str(
        "Return a summary with:\n\
         - Workspace-level memories written (list topics)\n\
         - Cross-project patterns discovered\n\
         - CLAUDE.md changes proposed/applied\n\
         - Any issues or gaps\n",
    );

    prompt
}

/// Build dispatch instructions for workspace onboarding.
#[allow(dead_code)]
pub(crate) fn build_workspace_instructions(
    main_prompt_path: &str,
    project_prompts: &[(String, String)],
    synthesis_path: &str,
    subagent_capable: bool,
) -> String {
    let p = main_prompt_path;

    if subagent_capable {
        let mut instructions = format!(
            "\
Onboarding required — this is a workspace with {} projects.

Step 1: Read prerequisites from the main prompt:
  read_markdown(\"{p}\", headings=[\"## Phase 0: Embedding Model Selection\", \"## Phase 1: Semantic Index Check\"])

Step 2: Spawn {} subagents IN PARALLEL — one per project:",
            project_prompts.len(),
            project_prompts.len(),
        );

        for (id, path) in project_prompts {
            instructions.push_str(&format!(
                "\n  - {id}: read_markdown(\"{path}\") and follow all instructions",
            ));
        }

        instructions.push_str(&format!(
            "\n\n\
Step 3: Wait for ALL subagents to complete.\n\n\
Step 4: Read the synthesis prompt and write workspace memories:\n\
  read_markdown(\"{synthesis_path}\")\n\n\
Follow the synthesis instructions to read back per-project memories,\n\
write workspace-level memories, generate the system prompt, and\n\
offer to refresh CLAUDE.md."
        ));

        instructions
    } else {
        let mut instructions = format!(
            "\
Onboarding required — this is a workspace with {} projects.

Step 1: Read prerequisites:
  read_markdown(\"{p}\", headings=[\"## Phase 0: Embedding Model Selection\", \"## Phase 1: Semantic Index Check\"])

Step 2: Explore each project one at a time:",
            project_prompts.len(),
        );

        for (id, path) in project_prompts {
            instructions.push_str(&format!(
                "\n  - {id}: read_markdown(\"{path}\") and follow all instructions",
            ));
        }

        instructions.push_str(&format!(
            "\n\n\
Step 3: Read the synthesis prompt and write workspace memories:\n\
  read_markdown(\"{synthesis_path}\")"
        ));

        instructions
    }
}
