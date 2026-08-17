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

/// The MEASURED client limit for MCP `initialize.instructions`, in **characters**.
///
/// Claude Code cuts the field at exactly 2048 chars. Measured 2026-08-16 by locating a
/// live session's own truncation point inside the rendered slice: the delivered prefix
/// ended mid-token at `- "symbol-navigatio`, which is byte 2092 and **char 2048** of
/// `build_server_instructions(None)`. 2048 is 2^11, not a coincidence.
///
/// The unit matters and was previously wrong. The old constant was named
/// `MAX_INSTRUCTIONS_CHARS` and compared against `String::len()`, which counts **bytes**
/// — and this surface is dense with em-dashes and arrows, so the same slice measured
/// 2127 bytes and 2081 chars. A byte budget over-counts the surface *and* the old value
/// (2200) sat above the real cliff, so the gate was wrong twice over while staying green.
///
/// See `docs/issues/archive/2026-08-15-server-instructions-truncated-before-reaching-the-model.md`.
pub(crate) const CLIENT_INSTRUCTIONS_CHAR_LIMIT: usize = 2048;

/// Characters held back from the measured cliff. The limit was observed on one client
/// build; another may cut a little lower, and a surface that arrives whole everywhere is
/// worth 48 characters.
pub(crate) const CHANNEL_SAFETY_MARGIN: usize = 48;

/// Build the full server instructions string, optionally appending dynamic project
/// status — **guaranteeing** the result fits the client channel.
///
/// The static slice is never sacrificed. Whatever does not fit is taken from the tail of
/// the dynamic block, at a line boundary, with the loss named. That inverts the previous
/// behaviour, where the client cut a fixed char count mid-token with no signal at all and
/// the `get_guide` pointer list — the mechanism by which a model discovers deeper
/// guidance exists — was exactly what fell off the end.
pub fn build_server_instructions(project_status: Option<&ProjectStatus>) -> String {
    let mut instructions = SERVER_INSTRUCTIONS.to_string();

    if let Some(status) = project_status {
        let dynamic = build_project_status_block(status);
        instructions.push_str(&fit_dynamic_block(&instructions, &dynamic));
    }

    instructions
}

/// Render the dynamic `## Project Status` suffix. Split out from
/// [`build_server_instructions`] so its length can be measured and trimmed before it is
/// appended, rather than discovered to be too long by a client that says nothing.
fn build_project_status_block(status: &ProjectStatus) -> String {
    /// Memory topic lists grow without bound (18 on this repo), and unbounded is exactly
    /// what a fixed channel cannot carry. The names are a pointer, not the content.
    const MAX_MEMORY_NAMES: usize = 8;

    let mut out = String::from("\n\n## Project Status\n\n");

    // "Active project" wording makes the implicit launch-time activation explicit —
    // agents see at a glance that activation happened without needing a separate tool
    // call signal. Pairs with the worktree line below so the activated root is never
    // ambiguous.
    out.push_str(&format!(
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
        // Explicit worktree banner — when present, the agent must NOT assume the
        // activated root is the canonical checkout. Changes here flow into commits,
        // branches, and PRs on the worktree's branch, not the main repo's.
        out.push_str(&format!("- **Worktree:** branch `{branch}` of `{main}`\n"));
    }

    if !status.languages.is_empty() {
        out.push_str(&format!(
            "- **Languages:** {}\n",
            status.languages.join(", ")
        ));
    }

    if !status.memories.is_empty() {
        // Bare list — the action verb is documented on the `memory` tool itself.
        let shown = status.memories.len().min(MAX_MEMORY_NAMES);
        let mut names = status.memories[..shown].join(", ");
        if status.memories.len() > shown {
            names.push_str(&format!(", +{} more", status.memories.len() - shown));
        }
        out.push_str(&format!("- **Memories:** {names}\n"));
    } else {
        out.push_str("- **Memories:** None yet — run `onboarding` to create project memories\n");
    }

    if status.has_index {
        out.push_str("- **Semantic index:** Built — `semantic_search` is ready to use\n");
    } else {
        out.push_str(
            "- **Semantic index:** Not built — run `index(action='build')` to enable `semantic_search`\n",
        );
    }

    // Workspace topology — inject project table when there are sibling projects.
    if let Some(projects) = &status.workspace {
        if !projects.is_empty() {
            out.push_str("\n## Workspace Projects\n\n");
            out.push_str("| Project | Root | Languages | Depends On |\n");
            out.push_str("|---------|------|-----------|------------|\n");
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
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    p.id, p.root, langs, deps
                ));
            }
            out.push_str(
                "\nUse `project: \"<id>\"` in `symbols` / `semantic_search` / `memory` to scope to a specific project.\n",
            );
        }
    }

    // Language-specific warnings — only injected when the project uses the language.
    if status.languages.iter().any(|l| l == "kotlin") {
        out.push_str(KOTLIN_KNOWN_ISSUES);
    }

    if let Some(prompt) = &status.system_prompt {
        out.push_str("\n\n## Custom Instructions\n\n");
        out.push_str(prompt);
        out.push('\n');
    }

    out
}

/// Trim `dynamic` so `static_part + dynamic` fits the channel, cutting at a **line**
/// boundary and naming the loss.
///
/// The client cuts from the tail at a fixed char count, mid-token, and says nothing — so
/// anything not fitted here vanishes silently. Dropping whole lines producer-side is
/// strictly better: the same content goes, but the agent learns that it went. Same
/// principle as `truncate_compact`'s head-placement fix and the librarian's promoted
/// overflow hints — when a channel truncates from the tail, ordering and an explicit
/// signal are the cheap fixes; the budget is the expensive one.
fn fit_dynamic_block(static_part: &str, dynamic: &str) -> String {
    const NOTE: &str = "- (status trimmed — it did not fit the MCP instructions channel)\n";

    let budget = CLIENT_INSTRUCTIONS_CHAR_LIMIT
        .saturating_sub(CHANNEL_SAFETY_MARGIN)
        .saturating_sub(static_part.chars().count());

    if dynamic.chars().count() <= budget {
        return dynamic.to_string();
    }

    let room = budget.saturating_sub(NOTE.chars().count());
    let mut kept = String::new();
    let mut used = 0usize;
    for line in dynamic.split_inclusive('\n') {
        let len = line.chars().count();
        if used + len > room {
            break;
        }
        kept.push_str(line);
        used += len;
    }
    kept.push_str(NOTE);
    kept
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
    "untrusted-content",
    "project-activation-bootstrap",
];

/// Guide topics reachable **only** by an explicit `get_guide(topic)` call, each with the
/// reason it has no `relevant_guide_topic()` trigger.
///
/// Authoring a guide and wiring its trigger are two separate edits, and nothing used to
/// prompt for the second. `src/prompts/README.md` rule 8 tells an author to move content
/// into a guide when `server_instructions` overflows its 2200-byte cap — so "move it to a
/// guide" read as *filed* when, absent a trigger, it is closer to *deleted from the
/// agent's view*. Measured 2026-08-16: 7 of 10 topics, 47,343 of 75,441 bytes — 63% of the
/// guide corpus — fired for nothing.
///
/// Being on this list is a **decision**, not a default. `every_guide_topic_is_triggered_or_declared_pull_only`
/// (`src/server.rs`) fails the build for any topic that is neither triggered nor listed
/// here, which is what stops the omission recurring silently. It also fails on a stale
/// entry — a topic listed here that later gains a trigger, or that no longer exists.
///
/// Entries marked `PENDING BL-25` are honest about the current state rather than
/// retrofitting a rationale: they are candidates for a trigger whose wiring is blocked on
/// a byte-budget decision, because `librarian` alone is 19.9 KB and already auto-injects on
/// a routine `artifact` call. Wiring more triggers before cutting the corpus trades one
/// problem for another.
///
/// See `docs/issues/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`.
pub const PULL_ONLY_GUIDE_TOPICS: &[(&str, &str)] = &[
    (
        "librarian-runtime",
        "By design. It is the spill-out half of `librarian` (19.9 KB, auto-injected), and \
         the parent guide ends by naming it. Wiring it would add 9.4 KB to a call that \
         already receives 19.9 KB, which is the byte problem this whole bug is about.",
    ),
    (
        "error-handling",
        "Authoring guidance for contributors writing a new tool — RecoverableError vs \
         anyhow::bail — not runtime guidance a caller acts on. No tool call implies it.",
    ),
    (
        "iron-laws-detail",
        "The gate text and its exceptions, expanding rules the always-loaded \
         `server_instructions` slice already states. A caller who needs the detail has \
         already read the pointer.",
    ),
    (
        "untrusted-content",
        "PENDING BL-25: not yet classified. The candidate trigger is whichever surface \
         first admits third-party text, which has not been identified.",
    ),
];

/// The guide that opens a session.
///
/// Delivered on the first guide-eligible tool call of a session, whatever that
/// call is — see `Tool::call_content` in `src/tools/core/types.rs`.
///
/// Before 2026-08-16 its only trigger was the `workspace` tool, so a session
/// that opened with `symbols`/`grep`/`read_file` — the common case — never
/// received it. That is a *discoverability* failure, not an adherence one: the
/// wording itself measured 100% plausibility-verified as eval arm `s1`
/// (prompt-engineering `scenarios/conclude-last`), and audit-log A-10 found that
/// on-demand guidance is obeyed as reliably as always-visible guidance once
/// fetched — its failure mode is "never fetched", not "fetched then forgotten".
/// So the fix is the trigger, not the text.
pub const SESSION_OPENING_GUIDE: &str = "project-activation-bootstrap";

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
        "untrusted-content" => Some(include_str!("guides/untrusted-content.md")),
        "project-activation-bootstrap" => {
            Some(include_str!("guides/project-activation-bootstrap.md"))
        }
        _ => None,
    }
}

/// The gate condition behind a refusal, for the error families where knowing it
/// is what prevents the next one.
///
/// Not a guide body — a **predicate**. `iron-laws-detail` holds the full text and
/// is 9.9 KB; these are ~150 bytes each and state the rule *and its exceptions*,
/// which is the part a refused caller cannot infer from the refusal itself.
///
/// **Why this exists.** Measured 2026-08-16 over codescout's own `usage.db`
/// (2026-07-17..2026-08-16): agents comply with an Iron-Law refusal on the very
/// next call **96%** of the time, and **47%** of sessions re-offend on the same
/// law later. They are not ignoring the gate — they cannot predict it, because
/// what makes an input refusable lives in `path_security.rs` and no surface
/// exposes it. `iron-laws-detail` does, and was fetched **once in 30 days
/// against 557 Iron-Law violations**: A-10's "never fetched" failure mode, since
/// guide injection sits after the `?` in `Tool::call_content` and so cannot ride
/// a refusal at all.
///
/// Delivered once per family per session via [`GuideLedger::notice_once`], which
/// keeps the key out of the topic namespace — a sentinel in `emitted` would
/// suppress [`SESSION_OPENING_GUIDE`].
///
/// (GF-4 / GF-5 in `docs/trackers/2026-08-16-iron-law-gate-firing-audit.md`.)
///
/// [`GuideLedger::notice_once`]: crate::tools::guide_ledger::GuideLedger::notice_once
pub fn refusal_predicate(err_family: &str) -> Option<&'static str> {
    Some(match err_family {
        "il1_read_overlaps_symbol" => {
            "IL-1 gate condition: a line-range read is refused only when the range OVERLAPS a \
             named symbol. Ranges over imports, `use` blocks, consts and other glue are allowed, \
             and `force=true` reads the range anyway. A whole-file read of source is always \
             refused — `symbols(path)` for the overview, \
             `symbols(name=..., include_body=true)` for one body."
        }
        "il2_structural_edit" => {
            "IL-2 gate condition: `edit_file` is refused when the edit spans a symbol DEFINITION \
             in a source file. Imports, string literals, comments and config are allowed. \
             Structural changes go through `edit_code` \
             (action=replace|insert|remove|rename)."
        }
        "il3_pipe_to_trimmer" => {
            "IL-3 gate condition: the check reads the LEFT side of the pipe. Unbounded producers \
             are cargo/npm/pnpm/yarn/python/pytest/go/mvn/gradle/rg/fd, recursive grep, and \
             `find` without -maxdepth. `git` is unbounded ONLY without an output limiter \
             (-n, --max-count, -3, --show-current, --porcelain/--short, --stat) — `--oneline` is \
             NOT a limiter, it bounds width rather than line count. On the RIGHT, aggregators \
             (wc, grep -c) are allowed; trimmers (head, tail, grep, sort) are not."
        }
        "il3_shell_on_source" => {
            "IL-3 source condition: refused when a CONTENT reader \
             (cat/head/tail/sed/awk/less/more/grep) names a source file INSIDE this project. \
             `wc` is allowed — it returns a count, not content. A path outside the project root \
             is allowed, because `symbols`/`read_file` resolve against the active project and \
             cannot serve it. `acknowledge_risk: true` bypasses."
        }
        _ => return None,
    })
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
    /// The worktree's **git name** — the `<name>` in
    /// `gitdir: <main>/.git/worktrees/<name>`. `None` only when the pointer
    /// does not have that shape.
    ///
    /// Git guarantees this is unique per repository, which the worktree's
    /// directory basename is not: `/a/wt` and `/b/wt` of the same repo share a
    /// basename but never share a git name. That distinction is load-bearing
    /// for `crate::retrieval::sync::worktree_ids`, which keys the delta index
    /// on it — two worktrees collapsing onto one delta project id means one
    /// worktree's sync prunes the other's chunks and then serves them from the
    /// wrong branch, classified `Healthy` with no warning.
    pub name: Option<String>,
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

    // The worktree's git name is the last segment of that same path. Git keeps
    // it unique per repo, so it is the only identifier here that two sibling
    // worktrees cannot share — see `WorktreeInfo::name`.
    let name = gitdir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty());

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

    Some(WorktreeInfo {
        branch,
        main_repo,
        name,
    })
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
                crate::util::fs::to_forward_slash(&p.relative_root),
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
    /// Retargeted 2026-08-16 (BL-9). The renderer's job — emit `## Custom Instructions`
    /// after the status lines — is unchanged and still asserted here. What changed is the
    /// *channel*: the MCP `instructions` field is capped at 2048 chars and a custom prompt
    /// does not fit alongside the static slice, so asserting on
    /// `build_server_instructions` would now be asserting that the trim is broken.
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
        let block = build_project_status_block(&status);
        assert!(block.contains("## Custom Instructions"));
        assert!(block.contains("Always use pytest."));
        // Custom instructions come after project status.
        let status_pos = block.find("## Project Status").unwrap();
        let custom_pos = block.find("## Custom Instructions").unwrap();
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
        // Asserted on the RENDERER, not on `build_server_instructions`: a workspace
        // table cannot fit the 2048-char MCP instructions channel alongside the static
        // slice, so the shipped surface trims it with a note (BL-9). Testing the whole
        // render here would be asserting that the trim is broken.
        let block = build_project_status_block(&status);
        assert!(block.contains("## Workspace Projects"));
        assert!(block.contains("mcp-server"));
        assert!(block.contains("python-services"));
        assert!(block.contains("python-services/"));
        // depends_on rendered for python-services
        assert!(block.contains("mcp-server"));
        // scoping hint present
        assert!(block.contains("project: \"<id>\""));
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
        // Renderer-level, for the same reason as the workspace-table test above: the
        // Kotlin block exceeds what the 2048-char instructions channel can carry next to
        // the static slice, so the shipped surface trims it with a note (BL-9).
        let block = build_project_status_block(&status);
        assert!(
            block.contains("kotlin-lsp"),
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
                name: Some("weekly-pattern".into()),
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
                name: Some("wt".into()),
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
        // The git worktree name is the last gitdir segment -- `feat` here,
        // deliberately DIFFERENT from the checkout directory's basename
        // (`wt`), so reading `name` off the wrong end of the path fails.
        // `retrieval::sync::worktree_key` keys the delta index on this.
        assert_eq!(info.name.as_deref(), Some("feat"));
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
    fn onboarding_prompts_write_system_prompt_to_root_not_memory() {
        // Regression (2026-06-12): onboarding must write the system prompt directly
        // to the root `.codescout/system-prompt.md` via `create_file` — that is the
        // always-on injection's read path (`Agent::project_status`). It must NOT route
        // through `memory(write, topic="system-prompt")`, which lands in
        // `.codescout/memories/` and never reaches `server_instructions`.
        // See docs/issues/archive/2026-06-12-onboarding-writes-system-prompt-to-memory-not-root.md.
        // The corrective prompts mention the prohibited call inside a "Do NOT" clause,
        // so we assert the POSITIVE `create_file` instruction plus absence of the
        // affirmative `topic: "system-prompt", content: ...)` form, not bare absence.
        for name in ["onboarding_prompt.md", "workspace_onboarding_prompt.md"] {
            let prompt = load_prompt(name);
            assert!(
                prompt.contains("create_file"),
                "{name} must instruct a direct `create_file` write of the system prompt"
            );
            assert!(
                !prompt.contains("topic: \"system-prompt\", content"),
                "{name} must NOT instruct memory(write, topic=\"system-prompt\", content=...) \
                 for the system prompt"
            );
        }
    }

    #[test]
    fn synthesis_prompt_writes_system_prompt_to_root_not_memory() {
        // Regression (2026-06-12): same root cause as the onboarding-prompt guard —
        // workspace synthesis writes the system prompt to the root file directly,
        // not via the memory store.
        let prompt =
            builders::build_synthesis_prompt(&[("proj-a".to_string(), vec!["rust".to_string()])]);
        assert!(
            prompt.contains(".codescout/system-prompt.md") && prompt.contains("create_file"),
            "synthesis prompt must instruct a direct create_file write to the root system-prompt.md"
        );
        assert!(
            !prompt.contains("topic=\"system-prompt\", content"),
            "synthesis prompt must NOT instruct memory(write, topic=\"system-prompt\", content=...)"
        );
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
        for &dead in DEPRECATED_TOOL_NAMES {
            assert!(
                !rendered.contains(dead),
                "rendered server instructions contains deprecated tool name: {dead}"
            );
        }
    }

    /// Tool names that have been removed/renamed and must never appear in any
    /// prompt surface the model reads. Single source of truth, shared by the
    /// rendered-`server_instructions` gate above and the `CLAUDE.md` gate below.
    const DEPRECATED_TOOL_NAMES: &[&str] = &[
        "find_symbol",
        "list_symbols",
        "replace_symbol",
        "insert_code",
        "rename_symbol",
        "search_pattern",
    ];

    /// `CLAUDE.md` is injected into every session as a `<system-reminder>` but is
    /// NOT one of the surfaces scanned by the rendered-instructions gate above
    /// or by `prompt_surfaces_reference_only_real_tools`. It is prose, so an
    /// allowlist guard is unusable here — denylist the known-dead names instead.
    /// (See refactor-log F-9 / F-10.)
    #[test]
    fn claude_md_contains_no_deprecated_tool_names() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/CLAUDE.md");
        let claude_md =
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
        for &dead in DEPRECATED_TOOL_NAMES {
            assert!(
                !claude_md.contains(dead),
                "CLAUDE.md references deprecated tool name: {dead}"
            );
        }
    }

    /// The `get_guide` bodies are the fourth prose surface the model reads, and
    /// until now the only one with no drift gate at all:
    /// `prompt_surfaces_reference_only_real_tools` builds its `surfaces` list from
    /// exactly three entries — `server_instructions.md`, `onboarding_prompt.md`,
    /// and `build_system_prompt_draft` — and no guide body appears in it. A guide
    /// is auto-injected into the session on the first call that triggers its
    /// topic, so a stale tool name there reaches the model exactly like one in
    /// `server_instructions` would.
    ///
    /// Denylist, for the same reason as the `CLAUDE.md` gate above: these are
    /// prose. Measured 2026-08-16 — the ten bodies carry 179 distinct backticked
    /// snake_case tokens against ~30 real tools, so an allowlist would need ~150
    /// non-tool entries, and the two-way tripwire on that list would make every
    /// guide edit a maintenance event. That is the trade F-9 left undecided.
    ///
    /// Iterates `GUIDE_TOPICS` rather than a hand-written list, so an eleventh
    /// guide is covered the moment it is registered — the failure mode this whole
    /// gate exists to prevent.
    ///
    /// (I-7 in `docs/trackers/test-escape-hardening.md`; friction F-9 in
    /// `docs/trackers/archive/prompt-guide-refactor-session-log.md`.)
    #[test]
    fn guide_bodies_contain_no_deprecated_tool_names() {
        for &topic in crate::prompts::GUIDE_TOPICS {
            let body = crate::prompts::topic_body(topic).unwrap_or_else(|| {
                panic!("GUIDE_TOPICS lists '{topic}' but topic_body returned None")
            });
            for &dead in DEPRECATED_TOOL_NAMES {
                assert!(
                    !body.contains(dead),
                    "get_guide body '{topic}' references deprecated tool name: {dead}"
                );
            }
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

    /// Maximum **character** length of the static `server_instructions` slice
    /// (`build_server_instructions(None)`).
    ///
    /// Sits below `CLIENT_INSTRUCTIONS_CHAR_LIMIT - CHANNEL_SAFETY_MARGIN` (2000) so the
    /// dynamic `## Project Status` block still has room to arrive. `fit_dynamic_block`
    /// guarantees the *total* never exceeds the channel; this budget is what keeps the
    /// dynamic half from being trimmed to nothing on every session.
    ///
    /// Two things about this constant were wrong before 2026-08-16 and are worth stating
    /// so they are not re-introduced. It was **2200**, above the measured 2048-char
    /// cliff — a cap set past the edge it exists to protect. And it was compared against
    /// `String::len()`, which counts **bytes**: the same slice measures 2127 bytes and
    /// 2081 chars, because the surface is dense with em-dashes and arrows. The gate was
    /// green throughout and the surface shipped truncated the whole time.
    ///
    /// If you need to add content, author a `get_guide(topic)` entry and reference it
    /// from the slice — do not raise this number.
    const STATIC_SLICE_CHAR_BUDGET: usize = 1900;

    #[test]
    fn source_md_under_cap() {
        let rendered = build_server_instructions(None);
        let chars = rendered.chars().count();
        assert!(
            chars <= STATIC_SLICE_CHAR_BUDGET,
            "static server instructions are {chars} chars ({} bytes); budget is {}. \
             Cut content or move it to get_guide — do not raise the budget. \
             NOTE the unit: the client cuts at {} CHARACTERS, not bytes.",
            rendered.len(),
            STATIC_SLICE_CHAR_BUDGET,
            crate::prompts::CLIENT_INSTRUCTIONS_CHAR_LIMIT,
        );
    }

    /// The gate the old one was not.
    ///
    /// `source_md_under_cap` measures `build_server_instructions(None)` — but **every**
    /// production call passes `Some(&status)`, which appends the whole Project Status
    /// block. Measured 2026-08-16: the bare render was 2127 bytes and passed the old
    /// 2200-byte cap, while the render that actually ships was 2350. The green test was
    /// measuring a string nobody receives.
    ///
    /// This is R-86's shape — *name every deployment mode the component has and ask
    /// which one the test constructed and which one production runs* — reached
    /// independently, from a cap rather than an LSP transport.
    #[test]
    fn production_render_fits_the_client_channel() {
        let status = ProjectStatus {
            name: "codescout".into(),
            path: "/home/marius/work/claude/codescout".into(),
            // Deliberately hostile: a real repo carries ~18 memory topics, and the list
            // is the one part of this block that grows without bound.
            languages: vec!["rust".into(), "kotlin".into(), "python".into()],
            memories: (0..30)
                .map(|i| format!("memory-topic-number-{i}"))
                .collect(),
            has_index: true,
            system_prompt: Some("A long custom instruction block. ".repeat(20)),
            workspace: None,
            worktree: None,
        };

        let rendered = build_server_instructions(Some(&status));
        let chars = rendered.chars().count();
        let ceiling =
            crate::prompts::CLIENT_INSTRUCTIONS_CHAR_LIMIT - crate::prompts::CHANNEL_SAFETY_MARGIN;
        assert!(
            chars <= ceiling,
            "production render is {chars} chars; the client cuts at {}. \
             The dynamic block must be trimmed to fit, never allowed to push the \
             static slice over the cliff.",
            crate::prompts::CLIENT_INSTRUCTIONS_CHAR_LIMIT,
        );

        // The static slice must survive INTACT — its last line is precisely what the
        // client used to cut, and losing it costs the model every `get_guide` pointer.
        assert!(
            rendered.contains("\"symbol-navigation\""),
            "the final static line must arrive whole; it is the one that was being cut"
        );
        assert!(
            rendered.contains("status trimmed"),
            "a trim must announce itself — the whole defect was that the loss was silent"
        );
    }

    /// A status block that fits must not be touched: the trim exists for the overflow
    /// case, and a note on every session would be noise that teaches nothing.
    #[test]
    fn a_status_block_that_fits_is_left_alone() {
        let status = ProjectStatus {
            name: "x".into(),
            path: "/tmp/x".into(),
            languages: vec!["rust".into()],
            memories: vec!["architecture".into(), "conventions".into()],
            has_index: true,
            system_prompt: None,
            workspace: None,
            worktree: None,
        };
        let rendered = build_server_instructions(Some(&status));
        assert!(rendered.contains("- **Memories:** architecture, conventions\n"));
        assert!(
            !rendered.contains("status trimmed"),
            "no trim note when nothing was trimmed"
        );
    }

    /// B-9 regression
    /// (docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md):
    /// the guide once claimed `cat src/foo.rs` was "allowed on bounded files".
    /// The gate is a *path* predicate (`check_source_file_access`, called from
    /// `src/tools/run_command/inner.rs` Step 2.5) with no per-command carve-out —
    /// measured 0/10 unaided survival against the false sentence
    /// (prompt-engineering scenarios/conclude-last, trap t2). The guide must state
    /// the block and the real override, and never re-grow the phantom carve-out.
    #[test]
    fn iron_laws_detail_gate_claim_matches_path_predicate() {
        let body = crate::prompts::topic_body("iron-laws-detail").expect("guide registered");
        assert!(
            !body.contains("allowed on bounded files"),
            "B-9 false claim resurfaced: the shell gate has no bounded-file carve-out"
        );
        assert!(body.contains("by path, not by command"));
        assert!(body.contains("acknowledge_risk: true"));
    }

    /// BL-26 regression: `artifact(action="move")` mints a NEW id — catalog
    /// identity is `sha256(abs_path)`, so a move cannot preserve it. What makes
    /// `move` the right call over delete+recreate is that it *grafts* the
    /// augmentation, events, links and observations onto the new id.
    ///
    /// The commit that made the graft work (`2d8c7f39`) corrected three of the
    /// four guide surfaces carrying the opposite claim and missed
    /// `librarian-runtime.md`. Scanning **every** registered guide, rather than the
    /// one that happened to drift, is the part that stops a fourth copy — the same
    /// reasoning as guarding every `edit_file` write path instead of the one that
    /// was reported.
    #[test]
    fn no_guide_claims_a_move_preserves_the_id() {
        for &topic in crate::prompts::GUIDE_TOPICS {
            let body = crate::prompts::topic_body(topic).expect("guide registered");
            for phrase in ["preserves `id`", "preserves the `id`", "preserves id"] {
                assert!(
                    !body.contains(phrase),
                    "guide '{topic}' says a move {phrase} — it mints a new one and grafts \
                     the history onto it. Say that instead; the response carries \
                     previous_id / id_changed."
                );
            }
        }
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

    /// IL1's always-loaded text must state the CONDITION, not just the permission.
    ///
    /// `read_file`'s gate refuses a source range whenever it overlaps a named symbol.
    /// A wording that says only "read_file is right for imports/glue" grants far more
    /// than the gate allows, and agents plan reads against the always-loaded text
    /// rather than the on-demand guide — which is accurate but must be asked for.
    /// That gap produced the largest single error class in the recorded corpus: 416
    /// refusals across 89 sessions, 4.7 per session
    /// (docs/issues/2026-08-15-il1-always-loaded-text-omits-the-overlap-condition.md).
    ///
    /// This is a REGRESSION guard, not a style check. The condition was authored,
    /// then deleted again by the slice refit that fitted the channel to 1900 chars
    /// (391fdcdc). That is the failure mode worth a test: the clause is 57 characters,
    /// it reads like filler to anyone counting budget, and its absence costs more
    /// than everything the refit saved. Whatever removes it should fail here first.
    #[test]
    fn il1_states_the_overlap_condition_not_just_the_permission() {
        let rendered = build_server_instructions(None);
        // Everything before Iron Law 2's marker is the header plus IL1.
        let il1 = rendered
            .split("2. NEVER")
            .next()
            .expect("server instructions must contain Iron Law 2");
        assert!(
            il1.contains("overlap"),
            "Iron Law 1 must state WHEN a line-range read is refused — that the range \
             overlaps a symbol — not merely that read_file is right for glue. Pay for \
             it elsewhere; do not drop the condition. IL1 read:\n{il1}"
        );
        assert!(
            il1.contains("force=true"),
            "Iron Law 1 must name the escape hatch beside the condition, or the \
             condition reads as a dead end. IL1 read:\n{il1}"
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
