//! Guard functions and utility predicates used across tools.

use super::types::{RecoverableError, ToolContext};

/// Block write operations when git worktrees exist but the agent hasn't
/// explicitly called `activate_project` to confirm which project to write to.
///
/// Returns `Ok(())` when writes are allowed:
/// - The call carries a `workspace=` pin, which names the target outright
/// - Agent chose this project during the session via `activate_project`
/// - No git worktrees exist (no ambiguity)
///
/// Returns `RecoverableError` when writes should be blocked:
/// - Worktrees exist AND the project was only resolved at startup, never chosen
///
/// Gates on [`Agent::is_project_chosen_this_session`], not
/// `is_project_explicitly_activated` — the latter is also true for the
/// `current_dir()` fallback in `run_server`, which fires in essentially every
/// session and is a default, not a choice. See BUG
/// docs/issues/archive/2026-08-16-worktree-write-guard-is-dead-code-in-production.md.
pub async fn guard_worktree_write(ctx: &ToolContext) -> anyhow::Result<()> {
    // A `workspace=` pin IS the answer to the question this guard asks. It also
    // resolves it more narrowly than `activate` does: the pin is per-call, so it
    // cannot be flipped mid-task by a peer or subagent the way the process-wide
    // session flag can (which is why `get_guide("workspace-state")` prescribes
    // pinning in preference to re-activating). Refusing a pinned write left the
    // caller who took that advice with no satisfiable remedy but the one the
    // docs warn against.
    //
    // Safe because the pin decides where the bytes land, not just what the
    // refusal message says: `resolve_write_or_capture` derives root, security
    // config and session write roots through `*_for(ctx.workspace_override)`,
    // and `call_tool_inner` acquires the PINNED project's write/file lock and
    // gates `check_tool_access` on the pinned security config. The pin cannot be
    // smuggled in either: `Server::build_context` is the only production
    // constructor of `ToolContext`, it sets this field solely from that call's
    // own `workspace` argument, and peer dispatch strips it
    // (`peer_tool_call_ignores_smuggled_workspace_override`).
    //
    // Checked first because it is the cheap test and does not await.
    // docs/issues/archive/2026-09-02-the-write-guard-refuses-a-correctly-pinned-call.md
    if ctx.workspace_override.is_some() {
        return Ok(());
    }
    if ctx.agent.is_project_chosen_this_session().await {
        return Ok(());
    }
    let root = ctx
        .agent
        .require_project_root_for(ctx.workspace_override.as_deref())
        .await?;
    let worktrees = crate::util::path_security::list_git_worktrees(&root);
    if worktrees.is_empty() {
        return Ok(());
    }
    let wt_list: Vec<String> = worktrees.iter().map(|p| p.display().to_string()).collect();
    let hint = format!(
        "Call workspace(action='activate', path=\"{}\") to select the write target (or use \"{}\" for the main repo).",
        wt_list[0],
        root.display()
    );
    Err(RecoverableError::with_hint(
        format!(
            "Write blocked: git worktrees detected but workspace(action='activate') has not been called. \
             Worktrees: [{}]",
            wt_list.join(", ")
        ),
        hint,
    )
    .into())
}

/// Returns true if the input looks like it was intended as a regex pattern
/// rather than a plain symbol name or literal text.
// Used by symbols and search_pattern.
pub(crate) fn is_regex_like(s: &str) -> bool {
    // Alternation: `foo|bar` but not `|leading` or `trailing|`
    if s.contains('|') {
        let parts: Vec<&str> = s.split('|').collect();
        if parts.iter().filter(|p| !p.is_empty()).count() >= 2 {
            return true;
        }
    }
    // Quantified wildcard: .* .+ .?
    if s.contains(".*") || s.contains(".+") || s.contains(".?") {
        return true;
    }
    // Anchors
    if s.starts_with('^') || s.ends_with('$') {
        return true;
    }
    // Character class with range: [A-Z] but not [u8]
    // Note: only inspects the first [...] pair in the string.
    if let Some(open) = s.find('[') {
        if let Some(close) = s[open..].find(']') {
            let inside = &s[open + 1..open + close];
            if inside.contains('-') && inside.len() > 2 {
                return true;
            }
        }
    }
    // Regex escape sequences
    if s.contains(r"\b") || s.contains(r"\w") || s.contains(r"\d") || s.contains(r"\s") {
        return true;
    }
    // Grouping: ( followed by )
    if let Some(open) = s.find('(') {
        if s[open..].contains(')') {
            return true;
        }
    }
    false
}
