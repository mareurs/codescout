//! Prompt templates for LLM guidance.
//!
//! Templates are stored as markdown files and compiled into the binary
//! via `include_str!`. Dynamic sections are appended at runtime based
//! on project state.

pub mod builders;
pub(crate) mod language_nav;

/// Static server instructions — tool reference, workflow patterns, steering rules.
pub const SERVER_INSTRUCTIONS: &str = include_str!("server_instructions.md");

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
    // Collect all languages across the active project and workspace peers.
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
    let nav_content = language_nav::render_symbol_navigation_block(&project_languages);
    let mut instructions = SERVER_INSTRUCTIONS.replace(SYMBOL_NAV_TOKEN, &nav_content);

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

pub const INCLUDE_MARKER: &str = "{{include: memory-templates.md}}";
pub const SYMBOL_NAV_TOKEN: &str = "{{symbol_navigation_block}}";

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
        load_prompt("workspace_onboarding_prompt.md")
    } else {
        load_prompt("onboarding_prompt.md")
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
    fn build_without_project_returns_substituted_static() {
        let result = build_server_instructions(None);
        // Token is substituted even with no project status.
        assert!(!result.contains("{{symbol_navigation_block}}"));
        assert!(result.contains("### Symbol Navigation Patterns"));
        // No per-language block when there are no languages.
        assert!(!result.contains("### Rust — Symbol Navigation"));
        assert!(!result.contains("## Project Status"));
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
        let prompt = load_prompt("onboarding_prompt.md");
        assert!(prompt.contains("## THE IRON LAW"));
        assert!(prompt.contains("## Phase 0: Embedding Model Selection"));
        assert!(prompt.contains("## Phase 1: Semantic Index Check"));
        assert!(prompt.contains("## Phase 2: Explore the Code"));
        assert!(prompt.contains("### project-scope: project-overview"));
        assert!(prompt.contains("### project-scope: architecture"));
        assert!(prompt.contains("## Coverage Verification"));
        assert!(prompt.contains("### Refresh CLAUDE.md"));
    }

    #[test]
    fn workspace_onboarding_prompt_contains_key_sections() {
        let prompt = load_prompt("workspace_onboarding_prompt.md");
        assert!(prompt.contains("# WORKSPACE MODE"));
        assert!(prompt.contains("## Phase 1 — Workspace Survey"));
        assert!(prompt.contains("## Phase 3 — Per-Project Deep Dives"));
        assert!(prompt.contains("## Phase 4 — Coverage Verification"));
        assert!(prompt.contains("## Phase 5 — Workspace Synthesis"));
        assert!(prompt.contains("## Phase 6 — CLAUDE.md Refresh"));
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

    #[test]
    fn memory_templates_have_all_project_scope_sections() {
        let templates = include_str!("memory-templates.md");
        for topic in [
            "project-overview",
            "architecture",
            "conventions",
            "development-commands",
            "domain-glossary",
            "gotchas",
        ] {
            let heading = format!("### project-scope: {topic}");
            assert!(
                templates.contains(&heading),
                "memory-templates.md missing heading: {heading}"
            );
        }
    }

    #[test]
    fn memory_templates_define_empty_stub() {
        let templates = include_str!("memory-templates.md");
        assert!(
            templates.contains("EMPTY_STUB:"),
            "memory-templates.md must define the canonical empty stub"
        );
    }

    #[test]
    fn memory_templates_have_all_workspace_scope_sections() {
        let templates = include_str!("memory-templates.md");
        for topic in [
            "architecture",
            "conventions",
            "development-commands",
            "domain-glossary",
            "gotchas",
            "system-prompt",
        ] {
            let heading = format!("### workspace-scope: {topic}");
            assert!(
                templates.contains(&heading),
                "memory-templates.md missing heading: {heading}"
            );
        }
    }

    #[test]
    fn workspace_architecture_template_has_required_subsections() {
        let templates = include_str!("memory-templates.md");
        for sub in [
            "Project Map",
            "Cross-Project Dependencies",
            "Shared Infrastructure",
            "Top-Level Code Map",
            "Generic Navigation",
        ] {
            assert!(
                templates.contains(&format!("- `## {sub}`")),
                "workspace architecture template missing required subsection: {sub}"
            );
        }
    }

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
            assert!(
                workspace.contains(phase),
                "workspace prompt missing phase: {phase}"
            );
        }
    }

    #[test]
    fn workspace_prompt_requires_six_memories_per_project() {
        let workspace = load_prompt("workspace_onboarding_prompt.md");
        assert!(
            workspace.contains("6 memories"),
            "workspace subagent prompt must require 6 memories per project"
        );
        for topic in [
            "project-overview",
            "architecture",
            "conventions",
            "development-commands",
            "domain-glossary",
            "gotchas",
        ] {
            assert!(
                workspace.contains(topic),
                "workspace prompt missing topic name: {topic}"
            );
        }
    }

    #[test]
    fn onboarding_prompt_uses_include_marker() {
        // The raw file (pre-substitution) must have the marker
        let raw = include_str!("onboarding_prompt.md");
        assert!(
            raw.contains("{{include: memory-templates.md}}"),
            "onboarding_prompt.md must contain the include marker"
        );
        // After load_prompt, marker is replaced by template content
        let loaded = load_prompt("onboarding_prompt.md");
        assert!(!loaded.contains("{{include:"));
        assert!(loaded.contains("### project-scope: project-overview"));
    }

    #[test]
    fn onboarding_prompt_phase_0_has_stable_heading_marker() {
        let raw = include_str!("onboarding_prompt.md");
        assert!(
            raw.contains("STABLE-HEADING"),
            "Phase 0 must carry a STABLE-HEADING comment to prevent cross-prompt drift"
        );
    }
    #[test]
    fn workspace_phase_0_reference_resolves() {
        let single = load_prompt("onboarding_prompt.md");
        let workspace = load_prompt("workspace_onboarding_prompt.md");
        let referenced = "## Phase 0: Embedding Model Selection";
        if workspace.contains(referenced) {
            assert!(
                single.contains(referenced),
                "workspace prompt references heading missing from single-project prompt"
            );
        }
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
}
