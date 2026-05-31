//! Guard functions and utility predicates used across tools.

use super::types::{RecoverableError, ToolContext};

/// Block write operations when git worktrees exist but the agent hasn't
/// explicitly called `activate_project` to confirm which project to write to.
///
/// Returns `Ok(())` when writes are allowed:
/// - Agent explicitly activated a project via `activate_project`
/// - No git worktrees exist (no ambiguity)
///
/// Returns `RecoverableError` when writes should be blocked:
/// - Worktrees exist AND the project was only implicitly set at startup
pub async fn guard_worktree_write(ctx: &ToolContext) -> anyhow::Result<()> {
    if ctx.agent.is_project_explicitly_activated().await {
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
