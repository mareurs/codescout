//! Prompt templates for LLM guidance.
//!
//! Templates are stored as markdown files and compiled into the binary
//! via `include_str!`. Dynamic sections are appended at runtime based
//! on project state.

pub mod builders;
pub mod source;

/// Static server instructions — tool reference, workflow patterns, steering rules.
pub const SERVER_INSTRUCTIONS: &str =
    include_str!(concat!(env!("OUT_DIR"), "/server_instructions.md"));

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
    let mut instructions = SERVER_INSTRUCTIONS.to_string();

    if let Some(status) = project_status {
        instructions.push_str("\n\n## Project Status\n\n");
        // "Active project" wording makes the implicit launch-time activation
        // explicit — agents see at a glance that activation happened without
        // needing a separate tool call signal. Pairs with the worktree line
        // below so the activated root is never ambiguous.
        instructions.push_str(&format!(
            "- **Active project:** {} at `{}`\n",
            status.name, status.path
        ));
        if let Some(wt) = &status.worktree {
            let branch = wt.branch.as_deref().unwrap_or("<detached HEAD>");
            let main = wt
                .main_repo
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            // Explicit worktree banner — when present, the agent must NOT
            // assume the activated root is the canonical checkout. Changes
            // here flow into commits, branches, and PRs on the worktree's
            // branch, not the main repo's.
            instructions.push_str(&format!("- **Worktree:** branch `{branch}` of `{main}`\n"));
        }
        if !status.languages.is_empty() {
            instructions.push_str(&format!(
                "- **Languages:** {}\n",
                status.languages.join(", ")
            ));
        }
        if !status.memories.is_empty() {
            // Bare list — the action verb is documented on the `memory` tool
            // itself. Keeping this short matters because the whole Project
            // Status block lands in Claude Code's ~2 KB instructions cut zone
            // (see docs/architecture/mcp-channel-caps.md); a long action-hint
            // suffix here would push the tail of the memory list off the cliff.
            instructions.push_str(&format!("- **Memories:** {}\n", status.memories.join(", ")));
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

/// Topic names registered as compiled-in `get_guide(topic)` content.
///
/// Single source of truth: `GetGuide` uses this for tool registration
/// and the input-schema enum; `Tool::call_content` uses [`topic_body`]
/// to inject the body when a `relevant_guide_topic()` hint fires.
pub const GUIDE_TOPICS: &[&str] = &[
    "librarian",
    "librarian-runtime",
    "tracker-conventions",
    "progressive-disclosure",
    "error-handling",
    "workspace-state",
    "iron-laws-detail",
    "symbol-navigation",
];

/// Return the compiled-in markdown body for a `get_guide(topic)` topic.
/// `None` for unknown topics — callers that need a hard-fail should match
/// `None` themselves; `GetGuide::call` wraps `None` into a
/// `RecoverableError`.
///
/// The matched cases must stay in sync with [`GUIDE_TOPICS`]; the
/// `prompts::tests::guide_topics_have_bodies` invariant enforces this.
pub fn topic_body(topic: &str) -> Option<&'static str> {
    match topic {
        "librarian" => Some(include_str!("guides/librarian.md")),
        "librarian-runtime" => Some(include_str!("guides/librarian-runtime.md")),
        "tracker-conventions" => Some(include_str!("guides/tracker-conventions.md")),
        "progressive-disclosure" => Some(include_str!("guides/progressive-disclosure.md")),
        "error-handling" => Some(include_str!("guides/error-handling.md")),
        "workspace-state" => Some(include_str!("guides/workspace-state.md")),
        "iron-laws-detail" => Some(include_str!("guides/iron-laws-detail.md")),
        "symbol-navigation" => Some(include_str!("guides/symbol-navigation.md")),
        _ => None,
    }
}

/// One row in the workspace project table injected into server instructions.
#[derive(Debug)]
pub struct WorkspaceProjectSummary {
    pub id: String,
    pub root: String,
    pub languages: Vec<String>,
    pub depends_on: Vec<String>,
}

/// Worktree context for the active project, when it lives in a git worktree
/// (i.e. `.git` is a *file* pointing at `<main_repo>/.git/worktrees/<name>/`,
/// not a regular `.git/` directory).
///
/// Used by [`build_server_instructions`] to surface a "Worktree: branch X of
/// /main/repo" line in the Project Status block so the agent knows when it's
/// operating in an isolated worktree vs the main checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    /// Current branch name parsed from the worktree's `HEAD` file. `None` when
    /// HEAD is detached (a raw SHA rather than a `ref: refs/heads/...` line).
    pub branch: Option<String>,
    /// Filesystem path of the main repo this worktree belongs to. Parsed from
    /// the `gitdir:` pointer in `.git` (the worktree's `.git` file contains
    /// `gitdir: <main>/.git/worktrees/<name>`; we strip the `/worktrees/<name>`
    /// suffix and a trailing `/.git` to recover `<main>`).
    pub main_repo: Option<std::path::PathBuf>,
}

/// Detect whether `root` is a git worktree and return basic context if so.
///
/// Returns `None` when:
/// - `<root>/.git` does not exist (not a git repo at all).
/// - `<root>/.git` is a directory (regular checkout — not a worktree).
/// - Reading the `.git` pointer file fails.
///
/// Filesystem-only — no `git` subprocess. The detection is the standard
/// "linked worktree" shape: `git worktree add` writes a `.git` *file*
/// containing `gitdir: <abs path to main repo's .git/worktrees/<name>>`.
pub fn detect_worktree_info(root: &std::path::Path) -> Option<WorktreeInfo> {
    let dot_git = root.join(".git");
    let meta = std::fs::symlink_metadata(&dot_git).ok()?;
    if !meta.file_type().is_file() {
        return None;
    }
    let pointer = std::fs::read_to_string(&dot_git).ok()?;
    let gitdir_line = pointer
        .lines()
        .find_map(|l| l.strip_prefix("gitdir:").map(str::trim))?;
    let gitdir = std::path::PathBuf::from(gitdir_line);

    // Recover the main repo path: gitdir typically looks like
    // `<main_repo>/.git/worktrees/<name>`. Strip `<name>` then `worktrees`
    // then `.git`. Be tolerant — if the shape doesn't match, we still
    // return a WorktreeInfo with main_repo: None.
    let main_repo = gitdir
        .parent() // .../.git/worktrees
        .and_then(|p| p.parent()) // .../.git
        .and_then(|p| p.parent()) // .../<main_repo>
        .map(std::path::PathBuf::from);

    // Branch comes from <gitdir>/HEAD: either `ref: refs/heads/<name>` or
    // a raw SHA (detached HEAD).
    let branch = std::fs::read_to_string(gitdir.join("HEAD"))
        .ok()
        .and_then(|s| {
            s.trim()
                .strip_prefix("ref: refs/heads/")
                .map(|b| b.trim().to_string())
        });

    Some(WorktreeInfo { branch, main_repo })
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
    /// Git worktree context for the active project. `Some(...)` when the
    /// project root lives in a linked git worktree (a `.git` *file* pointing
    /// at the main repo's worktree dir). Surfaced in server_instructions so
    /// the agent can tell worktree from main-checkout — see
    /// [`detect_worktree_info`].
    pub worktree: Option<WorktreeInfo>,
}

pub const INCLUDE_MARKER: &str = "{{include: memory-templates.md}}";

pub(crate) const RAW_ONBOARDING_PROMPT: &str =
    include_str!(concat!(env!("OUT_DIR"), "/onboarding_prompt.md"));
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
    fn build_with_project_appends_status() {
        let status = ProjectStatus {
            name: "my-project".into(),
            path: "/home/user/my-project".into(),
            languages: vec!["rust".into(), "python".into()],
            memories: vec!["architecture".into(), "conventions".into()],
            has_index: true,
            system_prompt: None,
            workspace: None,
            worktree: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(result.contains("## Project Status"));
        // D: explicit activation banner — "Active project" wording surfaces the
        // implicit launch-time activation so agents don't have to infer it from
        // path stripping in tool output.
        assert!(
            result.contains("**Active project:** my-project at `/home/user/my-project`"),
            "missing Active project banner, got:\n{result}"
        );
        assert!(result.contains("rust, python"));
        assert!(result.contains("architecture, conventions"));
        assert!(result.contains("Semantic index:** Built"));
        // Without worktree info present, NO worktree line should appear.
        assert!(
            !result.contains("Worktree:"),
            "non-worktree project must not emit a Worktree line, got:\n{result}"
        );
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
            worktree: None,
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
            worktree: None,
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
            worktree: None,
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
            worktree: None,
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
            worktree: None,
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
            worktree: None,
        };
        let result = build_server_instructions(Some(&status));
        assert!(
            result.contains("kotlin-lsp"),
            "Kotlin project must include Kotlin known issues"
        );
    }

    #[test]
    fn build_with_worktree_emits_worktree_banner() {
        // C: when ProjectStatus carries WorktreeInfo, the Project Status block
        // must surface a "Worktree: branch X of /main/repo" line so the agent
        // knows it's in a linked worktree, not the main checkout.
        let status = ProjectStatus {
            name: "backend-kotlin".into(),
            path: "/home/user/repo/.worktrees/weekly-pattern".into(),
            languages: vec!["kotlin".into()],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
            worktree: Some(WorktreeInfo {
                branch: Some("weekly-pattern".into()),
                main_repo: Some(std::path::PathBuf::from("/home/user/repo")),
            }),
        };
        let result = build_server_instructions(Some(&status));
        assert!(
            result.contains("**Worktree:** branch `weekly-pattern` of `/home/user/repo`"),
            "missing worktree banner, got:\n{result}"
        );
    }

    #[test]
    fn build_with_detached_worktree_renders_placeholder() {
        // Edge case: HEAD is detached (raw SHA, not `ref: refs/heads/...`).
        // The banner should still emit with a clear "<detached HEAD>" marker
        // rather than silently dropping the worktree line.
        let status = ProjectStatus {
            name: "wt".into(),
            path: "/some/path".into(),
            languages: vec![],
            memories: vec![],
            has_index: false,
            system_prompt: None,
            workspace: None,
            worktree: Some(WorktreeInfo {
                branch: None,
                main_repo: Some(std::path::PathBuf::from("/main")),
            }),
        };
        let result = build_server_instructions(Some(&status));
        assert!(
            result.contains("**Worktree:** branch `<detached HEAD>` of `/main`"),
            "detached HEAD placeholder missing, got:\n{result}"
        );
    }

    #[test]
    fn detect_worktree_info_identifies_linked_worktree() {
        // Build a fake worktree fixture on disk:
        //   <tmp>/main/.git/worktrees/feat/HEAD       — ref: refs/heads/feat
        //   <tmp>/wt/.git                              — gitdir: <tmp>/main/.git/worktrees/feat
        // detect_worktree_info(<tmp>/wt) must return Some with both branch
        // and main_repo populated correctly.
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main");
        let wt = dir.path().join("wt");
        let worktree_meta = main.join(".git").join("worktrees").join("feat");
        std::fs::create_dir_all(&worktree_meta).unwrap();
        std::fs::write(worktree_meta.join("HEAD"), "ref: refs/heads/feat\n").unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!("gitdir: {}\n", worktree_meta.display()),
        )
        .unwrap();

        let info = detect_worktree_info(&wt).expect("worktree should be detected");
        assert_eq!(info.branch.as_deref(), Some("feat"));
        assert_eq!(info.main_repo.as_deref(), Some(main.as_path()));
    }

    #[test]
    fn detect_worktree_info_returns_none_for_regular_checkout() {
        // A real checkout has `.git` as a directory, not a file. Detector
        // must return None so the banner stays absent.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        assert!(
            detect_worktree_info(dir.path()).is_none(),
            "regular checkout must not be classified as a worktree"
        );
    }

    #[test]
    fn detect_worktree_info_returns_none_when_no_git() {
        // Plain directory with no .git at all — defensive: returns None
        // rather than panicking on a missing path.
        let dir = tempfile::tempdir().unwrap();
        assert!(detect_worktree_info(dir.path()).is_none());
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
            worktree: None,
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
    fn guide_topics_have_bodies() {
        for &topic in crate::prompts::GUIDE_TOPICS {
            let body = crate::prompts::topic_body(topic).unwrap_or_else(|| {
                panic!(
                    "GUIDE_TOPICS lists '{topic}' but topic_body returned None — \
                     add a match arm with include_str!(\"guides/{topic}.md\")"
                )
            });
            assert!(!body.is_empty(), "topic '{topic}' has an empty body");
        }
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
        let raw = RAW_ONBOARDING_PROMPT;
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
        let raw = RAW_ONBOARDING_PROMPT;
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
            worktree: None,
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
        check_or_update_snapshot("onboarding_prompt.md", RAW_ONBOARDING_PROMPT);
    }

    #[test]
    fn prompt_surfaces_system_prompt_draft_empty_snapshot() {
        let draft = crate::prompts::builders::build_system_prompt_draft(&[], &[], None, None, &[]);
        check_or_update_snapshot("build_system_prompt_draft_empty.md", &draft);
    }
}

#[cfg(test)]
mod redesign_invariants {
    use super::*;

    /// Maximum byte length of the rendered `server_instructions` slice
    /// (`build_server_instructions(None)`). Claude Code silently truncates the
    /// MCP `initialize.instructions` field at ~2000 bytes — see
    /// `docs/architecture/mcp-channel-caps.md`. The 2200 cap gives ~200 bytes
    /// of headroom for the dynamic `## Project Status` block that runtime
    /// appends; growth beyond this risks truncating Iron Laws themselves
    /// rather than just the dynamic suffix. If you need to add content,
    /// author a `get_guide(topic)` entry and reference it from the slice.
    const MAX_INSTRUCTIONS_CHARS: usize = 2200;

    #[test]
    fn source_md_under_cap() {
        let rendered = build_server_instructions(None);
        assert!(
            rendered.len() <= MAX_INSTRUCTIONS_CHARS,
            "server instructions are {} chars; cap is {}. \
             Cut content or move it to get_guide.",
            rendered.len(),
            MAX_INSTRUCTIONS_CHARS,
        );
    }

    #[test]
    fn every_iron_law_has_do_instead() {
        let rendered = build_server_instructions(None);
        // Iron Laws section uses "NEVER X → Y" format. Each NEVER line must
        // have an arrow on the same line or within the next 2 lines.
        for (i, line) in rendered.lines().enumerate() {
            if line.contains("NEVER ")
                || line.starts_with(|c: char| c.is_ascii_digit()) && line.contains("NEVER")
            {
                let next_two: String = rendered
                    .lines()
                    .skip(i)
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ");
                assert!(
                    next_two.contains("→")
                        || next_two.contains(" use ")
                        || next_two.contains(" do "),
                    "Iron Law without do-instead clause: '{}'",
                    line
                );
            }
        }
    }

    #[test]
    fn server_instructions_mentions_get_guide() {
        let rendered = build_server_instructions(None);
        assert!(
            rendered.contains("get_guide"),
            "system prompt must mention get_guide for discoverability"
        );
    }

    #[test]
    fn server_instructions_does_not_concat_librarian() {
        // After Task 14 lands, the librarian block must not be appended.
        let rendered = build_server_instructions(None);
        assert!(
            !rendered.contains("artifact_event(action=\"create\")"),
            "librarian guide content should not be in instructions; \
             move it to get_guide(\"librarian\")"
        );
    }

    #[test]
    fn librarian_instructions_const_removed() {
        // Sentinel: any reintroduction of `crate::librarian::INSTRUCTIONS`
        // must remove this test or re-add the const. The presence of this
        // no-op test documents the deletion intent.
    }
}
