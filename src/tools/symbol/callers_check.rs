//! Decides whether `edit_code replace` may tell the caller that a symbol's callers
//! were checked: a changed declaration plus a call to the name in another file.
//!
//! The check is syntactic (a name match found by `grep`, classified as a call by the
//! tree-sitter call kinds), never resolved by a language server, and it under-counts
//! (Rust turbofish calls, calls inside macro arguments). So a clean result is "no call
//! seen", never "no callers exist", and only a *found* call is worth asserting.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

use super::edit_code::verify_callers_hint;
use crate::symbol::edit::declaration_changed;
use crate::tools::grep::Grep;
use crate::tools::symbol::call_edges::resolver::count_calls_named;
use crate::tools::{Tool, ToolContext};

/// Wall-clock budget for the whole grep and file-read loop; past it the answer is
/// `CannotTell`, so an edit never waits on a slow tree.
pub(super) const CALLERS_CHECK_BUDGET: Duration = Duration::from_millis(500);

/// Most candidate files read per check. More than this and a scan that found no call
/// is `CannotTell`, because the files left unread might hold one.
pub(super) const MAX_CANDIDATE_FILES: usize = 25;

/// What the check established about a replaced symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CallersCheck {
    /// The declaration header did not change (as far as the heuristic can tell).
    Unchanged,
    /// The header may have changed and no call to the name was seen outside the
    /// edited file. Syntactic and an under-count: not a claim that no callers exist.
    NoOutsideCalls,
    /// A call to the name, by syntax, in `file` (project-relative, forward slashes).
    OutsideCalls { file: String },
    /// The check could not decide: no header, no call classification for the
    /// language, an incomplete or failed search, too many candidates, or out of time.
    CannotTell,
}

/// Run the check for a replaced symbol `symbol_name` in `full_path` (a `language_id`
/// file), whose text went from `old_text` to `new_text`.
pub(super) async fn check_callers(
    ctx: &ToolContext,
    full_path: &Path,
    language_id: &str,
    symbol_name: &str,
    old_text: &str,
    new_text: &str,
) -> CallersCheck {
    match declaration_changed(old_text, new_text) {
        Some(false) => return CallersCheck::Unchanged,
        None => return CallersCheck::CannotTell,
        Some(true) => {}
    }
    within_budget(
        CALLERS_CHECK_BUDGET,
        scan_for_outside_call(ctx, full_path, language_id, symbol_name),
    )
    .await
}

/// `fut`, or `CannotTell` if it does not finish within `budget`.
pub(super) async fn within_budget<F: Future<Output = CallersCheck>>(
    budget: Duration,
    fut: F,
) -> CallersCheck {
    tokio::time::timeout(budget, fut)
        .await
        .unwrap_or(CallersCheck::CannotTell)
}

/// Grep for the name among files of the edited file's extension, then look for a call
/// to it in the best-ranked candidates other than the edited file.
async fn scan_for_outside_call(
    ctx: &ToolContext,
    full_path: &Path,
    language_id: &str,
    symbol_name: &str,
) -> CallersCheck {
    let Some(ext) = full_path.extension().and_then(|e| e.to_str()) else {
        return CallersCheck::CannotTell;
    };
    let Some(root) = ctx
        .agent
        .project_root_for(ctx.workspace_override.as_deref())
        .await
        .and_then(|r| std::fs::canonicalize(r).ok())
    else {
        return CallersCheck::CannotTell;
    };
    let input = json!({
        "pattern": format!(r"\b{}\b", regex::escape(symbol_name)),
        "mode": "files",
        "glob": format!("*.{ext}"),
    });
    let Ok(found) = Grep.call(input, ctx).await else {
        return CallersCheck::CannotTell;
    };
    if found.get("walk_cut").is_some() || found.get("completeness_warning").is_some() {
        return CallersCheck::CannotTell;
    }
    let Some(entries) = found.get("files").and_then(Value::as_array) else {
        return CallersCheck::CannotTell;
    };
    let edited = std::fs::canonicalize(full_path).unwrap_or_else(|_| full_path.to_path_buf());
    let candidates: Vec<PathBuf> = entries
        .iter()
        .filter_map(|e| e.get("file").and_then(Value::as_str))
        .map(|f| {
            let p = Path::new(f);
            let abs = if p.is_absolute() {
                p.to_path_buf()
            } else {
                root.join(p)
            };
            std::fs::canonicalize(&abs).unwrap_or(abs)
        })
        .filter(|p| *p != edited)
        .collect();

    for path in candidates.iter().take(MAX_CANDIDATE_FILES) {
        let Ok(text) = tokio::fs::read_to_string(path).await else {
            return CallersCheck::CannotTell;
        };
        match count_calls_named(&text, language_id, symbol_name) {
            None => return CallersCheck::CannotTell,
            Some(0) => {}
            Some(_) => {
                let rel = path.strip_prefix(&root).unwrap_or(path);
                return CallersCheck::OutsideCalls {
                    file: crate::util::fs::to_forward_slash(rel),
                };
            }
        }
    }
    if candidates.len() > MAX_CANDIDATE_FILES {
        CallersCheck::CannotTell
    } else {
        CallersCheck::NoOutsideCalls
    }
}

/// The `advice` object `edit_code` attaches for `check`, or `None` when there is
/// nothing to say. Only `OutsideCalls` is `verified`: the generic hint stays for
/// `CannotTell`, and a clean scan is silent because it is an under-count.
pub(super) fn advice_for(
    check: &CallersCheck,
    symbol_name: &str,
    name_path: &str,
    rel_path: &str,
) -> Option<Value> {
    let hint = verify_callers_hint(name_path, rel_path);
    match check {
        CallersCheck::OutsideCalls { file } => Some(json!({
            "text": format!(
                "{hint} | `{symbol_name}` is called by name in {file} (a syntactic match, not resolved by the language server)"
            ),
            "verified": true,
        })),
        CallersCheck::CannotTell => Some(json!({ "text": hint, "verified": false })),
        CallersCheck::Unchanged | CallersCheck::NoOutsideCalls => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::edit_code::verify_callers_hint;
    use super::*;
    use std::time::Duration;

    #[test]
    fn advice_for_a_found_call_is_verified_and_names_the_file() {
        let check = CallersCheck::OutsideCalls {
            file: "src/other.rs".to_string(),
        };
        let advice = advice_for(&check, "a", "a", "src/lib.rs").expect("a found call has advice");
        assert_eq!(advice["verified"], serde_json::json!(true));
        assert_eq!(
            advice["text"],
            serde_json::json!(format!(
                "{} | `a` is called by name in src/other.rs (a syntactic match, not resolved by the language server)",
                verify_callers_hint("a", "src/lib.rs")
            ))
        );
    }

    #[test]
    fn advice_for_cannot_tell_is_unverified_with_todays_text() {
        let advice = advice_for(&CallersCheck::CannotTell, "a", "Type/a", "src/lib.rs")
            .expect("cannot-tell keeps the generic hint");
        assert_eq!(advice["verified"], serde_json::json!(false));
        assert_eq!(
            advice["text"],
            serde_json::json!(verify_callers_hint("Type/a", "src/lib.rs"))
        );
    }

    #[test]
    fn advice_for_unchanged_and_no_outside_calls_is_none() {
        assert!(advice_for(&CallersCheck::Unchanged, "a", "a", "src/lib.rs").is_none());
        assert!(advice_for(&CallersCheck::NoOutsideCalls, "a", "a", "src/lib.rs").is_none());
    }

    #[tokio::test]
    async fn within_budget_maps_a_timeout_to_cannot_tell() {
        let out = within_budget(Duration::from_millis(10), std::future::pending()).await;
        assert_eq!(out, CallersCheck::CannotTell);
    }

    #[tokio::test]
    async fn within_budget_passes_a_result_through() {
        let out = within_budget(Duration::from_secs(5), async {
            CallersCheck::NoOutsideCalls
        })
        .await;
        assert_eq!(out, CallersCheck::NoOutsideCalls);
    }
}
