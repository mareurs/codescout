//! The note a `grep` result WITH matches carries when the walk pruned dot-prefixed entries at the
//! search root. The zero-match result already says so through `WalkAudit::completeness_warning`;
//! a non-zero result said nothing, so a partial list read as complete (measured 2026-10-31: a
//! search for `gate-slot` gave 23 files and no note, while `include_hidden=true` gave 27, among
//! them the only CI job that runs the suite).
//!
//! It is one short line, because it rides every non-zero search from a root that holds an
//! informative hidden entry, and on this repo that is most of them. It reuses the zero warning's
//! two predicates so the two halves cannot disagree about what was hidden:
//! `WalkAudit::hidden_at_root` (root entries only; `.git` and `.codescout` excluded as
//! uninformative) and `WalkAudit::globs_can_reach_a_root_dot_entry` (a glob anchored under a
//! visible directory cannot reach a root dot entry, so naming one would blame a directory the
//! query never touched). It is a separate key from `completeness_warning`, whose text opens with
//! "this zero" and would be false here.
//!
//! docs/issues/2026-10-31-a-grep-result-with-matches-is-silent-about-the-hidden-path-skip.md

use super::WalkAudit;

/// How many hidden root directories the note names before it counts the rest.
const NAMED: usize = 4;

/// The note for a result with matches, or `None` when nothing hidden was reachable by this query.
///
/// Names hidden DIRECTORIES only, unlike the zero warning. A pruned dotfile hides one file, and
/// nearly every repository has a root `.gitignore`, so counting dotfiles made the note fire on
/// almost every search in every project (the first version did, in this module's own bare-tree
/// test). A result that already has matches is the case where noise costs most and one
/// skipped file matters least; a pruned directory hides a whole subtree, which is the
/// `.github/` case the note exists for.
pub(super) fn hidden_skip_note(
    root: &std::path::Path,
    include_hidden: bool,
    globs: &[String],
) -> Option<String> {
    if include_hidden || !WalkAudit::globs_can_reach_a_root_dot_entry(globs) {
        return None;
    }
    let hidden: Vec<String> = WalkAudit::hidden_at_root(root)
        .into_iter()
        .filter(|name| name.ends_with('/'))
        .collect();
    if hidden.is_empty() {
        return None;
    }
    let shown: Vec<&str> = hidden.iter().take(NAMED).map(String::as_str).collect();
    let more = hidden.len() - shown.len();
    Some(format!(
        "Hidden paths at the search root were not searched: {}{}. Pass include_hidden=true to \
         search them.",
        shown.join(", "),
        if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        }
    ))
}
