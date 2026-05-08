//! Prompt templates for LLM guidance.
//!
//! Templates are stored as markdown files and compiled into the binary
//! via `include_str!`. Dynamic sections are appended at runtime based
//! on project state.

pub mod builders;
pub(crate) mod language_nav;
pub mod source;

/// Static server instructions — tool reference, workflow patterns, steering rules.
pub const SERVER_INSTRUCTIONS: &str =
    include_str!(concat!(env!("OUT_DIR"), "/server_instructions.md"));

/// Token in `server_instructions.md` replaced by the dynamic language nav block.
pub const SYMBOL_NAV_TOKEN: &str = "{{symbol_navigation_block}}";

/// Kotlin-specific known issues — only injected for projects with Kotlin.
const KOTLIN_KNOWN_ISSUES: &str = "\n\n## Language Support — Known Issues\n\n\
### Kotlin (kotlin-lsp)\n\n\
kotlin-lsp (JetBrains) has a **single workspace session** limitation: only one \
kotlin-lsp process can serve a given project directory at a time. If another \
codescout instance or editor is already running kotlin-lsp for the same project, \
new instances will fail with:\n\n\
> \"Multiple editing sessions for one workspace are not supported yet\"\n\n\
codescout detects this and fails fast with a clear error. **Workaround:** close \
the other session first, or use a single codescout instance for Kotlin projects.";

/// Build the full server instructions string, optionally appending
/// dynamic project status.
pub fn build_server_instructions(project_status: Option<&ProjectStatus>) -> String {
    // Collect all project language lists for the nav renderer.
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

    if let Some(status) = project_status {
        instructions.push_str("\n\n## Project Status\n\n");
        instructions.push_str(&format!(
            "- **Project:** {} at `{}`\n",
            status.name, status.path
        ));
        if !status.languages.is_empty() {
            instructions.push_str(&format!(
                "- **Languages:** {}\n",
                status.languages.join(", ")
            ));
        }
        if !status.memories.is_empty() {
            instructions.push_str(&format!(
                "- **Available shared memories:** {} — use `memory(action=\"read\", topic=...)` to read relevant ones as needed for your current task\n",
                status.memories.join(", ")
            ));
        } else {
            instructions.push_str(
                "- **Memories:** None yet — run `onboarding` to create project memories\n",
            );
        }
        if status.has_index {
            instructions
                .push_str("- **Semantic index:** Built — `semantic_search` is ready to use\n");
        } else {
            instructions.push_str(
                "- **Semantic index:** Not built — run `index(action='build')` to enable `semantic_search`\n",
            );
        }

        // Workspace topology — inject project table when there are sibling projects.
        if let Some(projects) = &status.workspace {
            if !projects.is_empty() {
                instructions.push_str("\n## Workspace Projects\n\n");
                instructions.push_str("| Project | Root | Languages | Depends On |\n");
                instructions.push_str("|---------|------|-----------|------------|\n");
                for p in projects {
                    let langs = if p.languages.is_empty() {
                        "—".to_string()
                    } else {
                        p.languages.join(", ")
                    };
                    let deps = if p.depends_on.is_empty() {
                        "—".to_string()
                    } else {
                        p.depends_on.join(", ")
                    };
                    instructions.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        p.id, p.root, langs, deps
                    ));
                }
                instructions.push_str(
                    "\nUse `project: \"<id>\"` in `symbols` / `semantic_search` / `memory` to scope to a specific project.\n",
                );
            }
        }

        // Language-specific warnings — only injected when the project uses the language.
        if status.languages.iter().any(|l| l == "kotlin") {
            instructions.push_str(KOTLIN_KNOWN_ISSUES);
        }

        if let Some(prompt) = &status.system_prompt {
            instructions.push_str("\n\n## Custom Instructions\n\n");
            instructions.push_str(prompt);
            instructions.push('\n');
        }
    }

    instructions
}

/// One row in the workspace project table injected into server instructions.
#[derive(Debug)]
pub struct WorkspaceProjectSummary {
    pub id: String,
    pub root: String,
    pub languages: Vec<String>,
    pub depends_on: Vec<String>,
}

/// Dynamic project status used to build server instructions.
#[derive(Debug)]
pub struct ProjectStatus {
    pub name: String,
    pub path: String,
    pub languages: Vec<String>,
    pub memories: Vec<String>,
    pub has_index: bool,
    pub system_prompt: Option<String>,
    /// Other projects in the workspace, if this is a multi-project repo.
    /// None for single-project activations; Some([]) is never emitted.
    pub workspace: Option<Vec<WorkspaceProjectSummary>>,
}

/// Onboarding prompt template — instructs Claude what to explore and what memories to create.
pub const ONBOARDING_PROMPT: &str = include_str!(concat!(env!("OUT_DIR"), "/onboarding_prompt.md"));

/// Workspace-specific onboarding prompt — appended when multiple projects are discovered.
pub const WORKSPACE_ONBOARDING_PROMPT: &str = include_str!("workspace_onboarding_prompt.md");

pub const INCLUDE_MARKER: &str = "{{include: memory-templates.md}}";

const MEMORY_TEMPLATES: &str = include_str!("memory-templates.md");

/// Load a prompt with `{{include: memory-templates.md}}` markers substituted.
pub fn load_prompt(name: &str) -> String {
    let raw = match name {
        "onboarding_prompt.md" => ONBOARDING_PROMPT,
        "workspace_onboarding_prompt.md" => WORKSPACE_ONBOARDING_PROMPT,
        other => panic!("unknown prompt: {other}"),
    };
    raw.replace(INCLUDE_MARKER, MEMORY_TEMPLATES)
}

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

/// Build the onboarding prompt, substituting detected project information.
///
/// In workspace mode (multiple projects discovered) the single-project
/// `ONBOARDING_PROMPT` is omitted entirely — keeping its Phase 1/Phase 2
/// instructions in the prompt caused orchestrators to spawn an extra "root"
/// subagent in addition to the per-project ones, duplicating exploration of
/// the dominant sub-project.
pub fn build_onboarding_prompt(ctx: &OnboardingContext) -> String {
    let workspace_mode = ctx.is_workspace && ctx.projects.len() > 1;

    let mut prompt = if workspace_mode {
        WORKSPACE_ONBOARDING_PROMPT.to_string()
    } else {
        ONBOARDING_PROMPT.to_string()
    };

    prompt.push_str("\n\n---\n\n");

    if !ctx.languages.is_empty() {
        prompt.push_str(&format!(
            "**Detected languages:** {}\n\n",
            ctx.languages.join(", ")
        ));
    }

    if !ctx.top_level.is_empty() {
        prompt.push_str(&format!(
            "**Top-level structure:**\n```\n{}\n```\n\n",
            ctx.top_level.join("\n")
        ));
    }

    if !ctx.entry_points.is_empty() {
        prompt.push_str(&format!(
            "**Entry points found:** {}\n\n",
            ctx.entry_points.join(", ")
        ));
    }

    if !ctx.test_dirs.is_empty() {
        prompt.push_str(&format!(
            "**Test directories:** {}\n\n",
            ctx.test_dirs.join(", ")
        ));
    }

    if !ctx.ci_files.is_empty() {
        prompt.push_str(&format!(
            "**CI config files:** {}\n\n",
            ctx.ci_files.join(", ")
        ));
    }

    if !ctx.key_files.is_empty() {
        prompt.push_str(&format!(
            "**Key files to read during Phase 1:**\n{}\n\n",
            ctx.key_files
                .iter()
                .map(|f| format!("- `{f}`"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    if ctx.index_ready {
        prompt.push_str(&format!(
            "**Semantic index:** ready ({} files, {} chunks)\n\n",
            ctx.index_files, ctx.index_chunks
        ));
    } else {
        prompt.push_str("**Semantic index:** not built\n\n");
    }

    if workspace_mode {
        prompt.push_str(&format!(
            "**Workspace mode:** {} projects detected\n\n",
            ctx.projects.len()
        ));
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

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_instructions_contain_key_sections() {
        assert!(SERVER_INSTRUCTIONS.contains("## Tool Routing & Gotchas"));
        assert!(SERVER_INSTRUCTIONS.contains("## Output System"));
        assert!(SERVER_INSTRUCTIONS.contains("## Rules"));
    }

    #[test]
    fn build_without_project_returns_static() {
        let result = build_server_instructions(None);
        // Token is substituted even with no project status.
        assert!(!result.contains("{{symbol_navigation_block}}"));
        assert!(result.contains("### Symbol Navigation Patterns"));
        // No per-language block when there are no languages.
        assert!(!result.contains("### Rust — Symbol Navigation"));
        assert!(!result.contains("## Project Status"));
    }

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
        assert!(
            !result.contains("{{symbol_navigation_block}}"),
            "token must be substituted in build_server_instructions output"
        );
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

    #[test]
    fn rendered_server_instructions_contains_no_deprecated_tool_names() {
        let status = ProjectStatus {
            name: "x".into(),
            path: "/tmp/x".into(),
            languages: vec![
                "rust".into(),
                "python".into(),
                "typescript".into(),
                "kotlin".into(),
                "go".into(),
            ],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
        };
        let rendered = build_server_instructions(Some(&status));
        for dead in [
            "find_symbol",
            "list_symbols",
            "replace_symbol",
            "insert_code",
            "rename_symbol",
            "search_pattern",
        ] {
            assert!(
                !rendered.contains(dead),
                "rendered server instructions contains deprecated tool name: {dead}"
            );
        }
    }

    #[test]
    fn iron_law_8_promotes_call_graph_before_references() {
        let raw = SERVER_INSTRUCTIONS;
        let idx = raw
            .find("8. **CALL GRAPH BEFORE STRUCTURAL EDITS.**")
            .expect("Iron Law 8 must be the call_graph promotion");
        let body = &raw[idx..idx.saturating_add(500)];
        let cg = body.find("call_graph").expect("call_graph must appear");
        let refs = body.find("references").expect("references must appear");
        assert!(cg < refs, "call_graph must be named before references");
    }

    #[test]
    fn impact_analysis_section_contains_call_graph_with_full_arguments() {
        let raw = SERVER_INSTRUCTIONS;
        let section_start = raw.find("### Impact Analysis").expect("section must exist");
        let next = raw[section_start..]
            .find("\n### ")
            .map(|i| section_start + i)
            .unwrap_or(raw.len());
        let section = &raw[section_start..next];

        assert!(
            section.contains("call_graph(symbol="),
            "Impact Analysis must include a call_graph call with named symbol arg"
        );
        assert!(
            section.contains("direction=\"callers\""),
            "Impact Analysis must demonstrate direction=\"callers\""
        );
        assert!(
            section.contains("max_depth=3"),
            "Impact Analysis must demonstrate max_depth=3"
        );
        assert!(
            section.contains("`references`"),
            "Impact Analysis must reference the references tool"
        );
    }

    #[test]
    fn build_with_project_appends_status() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/home/user/my-project".into(),
            languages: vec!["rust".into(), "python".into()],
            memories: vec!["architecture".into(), "conventions".into()],
            has_index: true,
            system_prompt: None,
            workspace: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(result.contains("## Project Status"));
        assert!(result.contains("my-project"));
        assert!(result.contains("rust, python"));
        assert!(result.contains("architecture, conventions"));
        assert!(result.contains("Semantic index:** Built"));
    }

    #[test]
    fn build_with_no_memories_suggests_onboarding() {
        let status = ProjectStatus {
            name: "new-project".into(),
            path: "/tmp/new".into(),
            languages: vec![],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(result.contains("run `onboarding`"));
        assert!(result.contains("run `index(action='build')`"));
    }

    #[test]
    fn onboarding_prompt_contains_key_sections() {
        assert!(ONBOARDING_PROMPT.contains("### Rules"));
        assert!(ONBOARDING_PROMPT.contains("### Memories to Create"));
        assert!(ONBOARDING_PROMPT.contains("project-overview"));
        assert!(ONBOARDING_PROMPT.contains("architecture"));
        assert!(ONBOARDING_PROMPT.contains("conventions"));
        assert!(ONBOARDING_PROMPT.contains("development-commands"));
        assert!(ONBOARDING_PROMPT.contains("domain-glossary"));
        assert!(ONBOARDING_PROMPT.contains("gotchas"));
        assert!(ONBOARDING_PROMPT.contains("## Gathered Project Data"));
        // Verify enforcement sections exist
        assert!(ONBOARDING_PROMPT.contains("## Phase 1: Semantic Index Check"));
        assert!(ONBOARDING_PROMPT.contains("## THE IRON LAW"));
        assert!(ONBOARDING_PROMPT.contains("<HARD-GATE>"));
        assert!(ONBOARDING_PROMPT.contains("## Red Flags"));
        assert!(ONBOARDING_PROMPT.contains("## Common Rationalizations"));
    }

    #[test]
    fn workspace_onboarding_prompt_contains_key_sections() {
        assert!(WORKSPACE_ONBOARDING_PROMPT.contains("Workspace Survey"));
        assert!(WORKSPACE_ONBOARDING_PROMPT.contains("Workspace Deep Dives"));
        assert!(WORKSPACE_ONBOARDING_PROMPT.contains("Phase 2"));
        assert!(WORKSPACE_ONBOARDING_PROMPT.contains("Subagent"));
        assert!(WORKSPACE_ONBOARDING_PROMPT.contains("HARD-GATE"));
        assert!(WORKSPACE_ONBOARDING_PROMPT.contains("Re-Onboarding"));
    }

    #[test]
    fn build_onboarding_includes_languages() {
        let result = build_onboarding_prompt(&OnboardingContext {
            languages: &["rust".into(), "python".into()],
            top_level: &["src/".into(), "tests/".into()],
            key_files: &[],
            ci_files: &[],
            entry_points: &[],
            test_dirs: &[],
            index_ready: false,
            index_files: 0,
            index_chunks: 0,
            projects: &[],
            is_workspace: false,
        });
        assert!(result.contains("rust, python"));
        assert!(result.contains("src/"));
    }

    #[test]
    fn build_onboarding_handles_empty() {
        let result = build_onboarding_prompt(&OnboardingContext {
            languages: &[],
            top_level: &[],
            key_files: &[],
            ci_files: &[],
            entry_points: &[],
            test_dirs: &[],
            index_ready: false,
            index_files: 0,
            index_chunks: 0,
            projects: &[],
            is_workspace: false,
        });
        assert!(result.contains("## Rules"));
        assert!(!result.contains("Detected languages"));
    }

    #[test]
    fn build_onboarding_includes_gathered_context() {
        let result = build_onboarding_prompt(&OnboardingContext {
            languages: &["rust".into(), "python".into()],
            top_level: &["src/".into(), "tests/".into()],
            key_files: &["README.md".into(), "Cargo.toml".into(), "CLAUDE.md".into()],
            ci_files: &[".github/workflows/ci.yml".into()],
            entry_points: &["src/main.rs".into()],
            test_dirs: &["tests".into()],
            index_ready: false,
            index_files: 0,
            index_chunks: 0,
            projects: &[],
            is_workspace: false,
        });
        assert!(result.contains("Cargo.toml"));
        assert!(result.contains("ci.yml"));
        assert!(result.contains("src/main.rs"));
        assert!(result.contains("Detected languages"));
    }

    #[test]
    fn build_with_system_prompt_appends_custom_section() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            languages: vec![],
            memories: vec![],
            has_index: false,
            system_prompt: Some("Always use pytest.".into()),
            workspace: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(result.contains("## Custom Instructions"));
        assert!(result.contains("Always use pytest."));
        // Custom instructions should come after project status
        let status_pos = result.find("## Project Status").unwrap();
        let custom_pos = result.find("## Custom Instructions").unwrap();
        assert!(custom_pos > status_pos);
    }

    #[test]
    fn build_without_system_prompt_has_no_custom_section() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            languages: vec![],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(!result.contains("## Custom Instructions"));
    }

    #[test]
    fn build_with_workspace_appends_project_table() {
        let status = ProjectStatus {
            name: "backend-kotlin".into(),
            path: "/workspace/backend-kotlin".into(),
            languages: vec!["kotlin".into()],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: Some(vec![
                WorkspaceProjectSummary {
                    id: "backend-kotlin".into(),
                    root: ".".into(),
                    languages: vec!["kotlin".into()],
                    depends_on: vec![],
                },
                WorkspaceProjectSummary {
                    id: "mcp-server".into(),
                    root: "mcp-server/".into(),
                    languages: vec!["typescript".into()],
                    depends_on: vec![],
                },
                WorkspaceProjectSummary {
                    id: "python-services".into(),
                    root: "python-services/".into(),
                    languages: vec!["python".into()],
                    depends_on: vec!["mcp-server".into()],
                },
            ]),
        };
        let result = build_server_instructions(Some(&status));
        assert!(result.contains("## Workspace Projects"));
        assert!(result.contains("mcp-server"));
        assert!(result.contains("python-services"));
        assert!(result.contains("python-services/"));
        // depends_on rendered for python-services
        assert!(result.contains("mcp-server"));
        // scoping hint present
        assert!(result.contains("project: \"<id>\""));
    }

    #[test]
    fn build_with_single_project_no_workspace_table() {
        // workspace: None → no table emitted even if the field is absent
        let status = ProjectStatus {
            name: "solo".into(),
            path: "/solo".into(),
            languages: vec!["rust".into()],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(!result.contains("## Workspace Projects"));
    }

    #[test]
    fn build_onboarding_shows_index_ready() {
        let result = build_onboarding_prompt(&OnboardingContext {
            languages: &["rust".into()],
            top_level: &[],
            key_files: &[],
            ci_files: &[],
            entry_points: &[],
            test_dirs: &[],
            index_ready: true,
            index_files: 42,
            index_chunks: 350,
            projects: &[],
            is_workspace: false,
        });
        assert!(result.contains("Semantic index:** ready (42 files, 350 chunks)"));
    }

    #[test]
    fn build_onboarding_shows_index_not_built() {
        let result = build_onboarding_prompt(&OnboardingContext {
            languages: &["rust".into()],
            top_level: &[],
            key_files: &[],
            ci_files: &[],
            entry_points: &[],
            test_dirs: &[],
            index_ready: false,
            index_files: 0,
            index_chunks: 0,
            projects: &[],
            is_workspace: false,
        });
        assert!(result.contains("Semantic index:** not built"));
    }

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
        assert!(prompt.contains("Workspace Survey"));
        assert!(prompt.contains("api"));
        assert!(prompt.contains("frontend"));
    }

    #[test]
    fn build_with_kotlin_project_includes_kotlin_warnings() {
        let status = ProjectStatus {
            name: "test".into(),
            path: "/tmp/test".into(),
            languages: vec!["kotlin".into(), "java".into()],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(
            result.contains("kotlin-lsp"),
            "Kotlin project must include Kotlin known issues"
        );
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
            workspace: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(
            !result.contains("kotlin-lsp"),
            "Non-Kotlin project must not include Kotlin known issues"
        );
    }

    // ---------- Y-C: surface roundtrip snapshots (gates I-01) ----------
    //
    // These tests pin the rendered output of the three prompt surfaces so the
    // I-01 refactor (consolidating into a single `source.md` template) can
    // prove zero content drift. Regenerate intentionally with:
    //   UPDATE_PROMPT_SNAPSHOTS=1 cargo test --lib prompt_surfaces

    fn fixture_path(name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/prompt_surfaces")
            .join(name)
    }

    fn check_or_update_snapshot(name: &str, current: &str) {
        let path = fixture_path(name);
        if std::env::var("UPDATE_PROMPT_SNAPSHOTS").is_ok() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create fixture dir");
            }
            std::fs::write(&path, current).expect("write fixture");
            eprintln!("updated snapshot: {}", path.display());
            return;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "missing snapshot `{}`: {e}\n\
                 Regenerate with: UPDATE_PROMPT_SNAPSHOTS=1 cargo test --lib prompt_surfaces",
                path.display()
            )
        });
        if expected != current {
            panic!(
                "prompt surface drift in `{name}`\n  \
                 expected: {} bytes\n  \
                 actual:   {} bytes\n\n\
                 If intentional, regenerate with:\n\
                 \x20 UPDATE_PROMPT_SNAPSHOTS=1 cargo test --lib prompt_surfaces\n\n\
                 Otherwise this is a regression — I-01 (and any later prompt-template\n\
                 refactor) must preserve rendered content byte-for-byte.",
                expected.len(),
                current.len()
            );
        }
    }

    #[test]
    fn prompt_surfaces_server_instructions_snapshot() {
        check_or_update_snapshot("server_instructions.md", SERVER_INSTRUCTIONS);
    }

    #[test]
    fn prompt_surfaces_onboarding_snapshot() {
        check_or_update_snapshot("onboarding_prompt.md", ONBOARDING_PROMPT);
    }

    #[test]
    fn prompt_surfaces_system_prompt_draft_empty_snapshot() {
        let draft = crate::prompts::builders::build_system_prompt_draft(&[], &[], None, None, &[]);
        check_or_update_snapshot("build_system_prompt_draft_empty.md", &draft);
    }

    #[test]
    fn load_prompt_substitutes_include_marker() {
        let single = load_prompt("onboarding_prompt.md");
        let workspace = load_prompt("workspace_onboarding_prompt.md");
        assert!(
            !single.contains("{{include: memory-templates.md}}"),
            "include marker should be substituted in single-project prompt"
        );
        assert!(
            !workspace.contains("{{include: memory-templates.md}}"),
            "include marker should be substituted in workspace prompt"
        );
    }
}
