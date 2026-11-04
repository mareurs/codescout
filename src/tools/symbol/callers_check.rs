//! Decides whether `edit_code replace` may tell the caller that a symbol's callers
//! were checked: a changed declaration plus a call to the name in another file.
//!
//! The check is syntactic (a name match found by `grep`, classified as a call by the
//! tree-sitter call kinds), never resolved by a language server. It under-counts:
//! Rust turbofish calls (`f::<T>(..)`) and calls inside macro arguments are not seen,
//! and macro invocations (`warn!(..)`) are ignored by design. So a clean result is
//! "no call seen", never "no callers exist", and only a *found* call is worth
//! asserting.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Value};

use super::edit_code::verify_callers_hint;
use crate::symbol::edit::declaration_changed;
use crate::tools::grep::Grep;
use crate::tools::symbol::call_edges::resolver::{classifies_calls, count_calls_named};
use crate::tools::{Tool, ToolContext};

/// Wall-clock budget for the whole grep and file-read loop; past it the answer is
/// `CannotTell`, so an edit never waits on a slow tree.
// cap-class: NOT_A_CAP — a deadline on a check, not a bound on a result; running over it yields `CannotTell`, which `advice_for` renders as explicit unverified advice, so nothing partial is presented as complete
pub(super) const CALLERS_CHECK_BUDGET: Duration = Duration::from_millis(500);

/// Most candidate files read per check. More than this and a scan that found no call
/// is `CannotTell`, because the files left unread might hold one.
// cap-class: NOT_A_CAP — bounds how many files the check reads, not what it returns; past it with no call found the verdict is `CannotTell`, rendered by `advice_for` as unverified advice, so no partial scan is presented as complete
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
    // Without call classification no file can ever show a call, so searching is moot.
    if !classifies_calls(language_id) {
        return CallersCheck::CannotTell;
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
    let Ok(found) = Grep.call(grep_input(symbol_name, ext), ctx).await else {
        return CallersCheck::CannotTell;
    };
    let Some(entries) = listed_candidates(&found) else {
        return CallersCheck::CannotTell;
    };
    let edited = std::fs::canonicalize(full_path).unwrap_or_else(|_| full_path.to_path_buf());
    // Lazy: canonicalise in ranked order and stop one past the cap, which is all the
    // cap decision needs.
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
        .take(MAX_CANDIDATE_FILES + 1)
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
    if candidates.len() > MAX_CANDIDATE_FILES || !callers_share_one_extension(language_id) {
        CallersCheck::CannotTell
    } else {
        CallersCheck::NoOutsideCalls
    }
}

/// The `Grep` input that lists files mentioning `symbol_name` among `*.<extension>`.
/// `time_budget_ms` is Grep's own walk clock: its files-mode walk is synchronous, so
/// only this can stop it (a tokio timeout cannot), and an overrun sets `walk_cut`.
fn grep_input(symbol_name: &str, extension: &str) -> Value {
    json!({
        "pattern": format!(r"\b{}\b", regex::escape(symbol_name)),
        "mode": "files",
        "glob": format!("*.{extension}"),
        "time_budget_ms": CALLERS_CHECK_BUDGET.as_millis() as u64,
    })
}

/// The ranked `files` entries of a files-mode `Grep` result, or `None` when the list
/// cannot be trusted as complete: a cut walk, a completeness warning, or no array.
fn listed_candidates(found: &Value) -> Option<&Vec<Value>> {
    if found.get("walk_cut").is_some() || found.get("completeness_warning").is_some() {
        return None;
    }
    found.get("files")?.as_array()
}

/// Whether a language's callers live in files of one extension, so the single-extension
/// glob in `grep_input` sees them all. For the others (`.ts` vs `.tsx`/`.mts`, `.js` vs
/// `.jsx`/`.mjs`, `.kt` vs `.kts`/Java) "no call found" is not a definite no; sibling
/// extensions are a later slice.
fn callers_share_one_extension(language_id: &str) -> bool {
    matches!(language_id, "rust" | "python")
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

    #[test]
    fn grep_input_carries_the_escaped_pattern_mode_glob_and_walk_budget() {
        let input = grep_input("a.b", "rs");
        assert_eq!(input["pattern"], json!(r"\ba\.b\b"));
        assert_eq!(input["mode"], json!("files"));
        assert_eq!(input["glob"], json!("*.rs"));
        assert_eq!(
            input["time_budget_ms"],
            json!(CALLERS_CHECK_BUDGET.as_millis() as u64),
            "Grep's own walk clock must carry the budget, a tokio timeout cannot cut its sync walk"
        );
    }

    #[test]
    fn listed_candidates_accepts_a_clean_result() {
        let found = json!({"files": [{"file": "a.rs", "count": 1}], "total": 1});
        assert_eq!(listed_candidates(&found).map(|f| f.len()), Some(1));
    }

    #[test]
    fn listed_candidates_rejects_a_cut_walk() {
        let found = json!({"files": [], "total": 0, "walk_cut": "budget ran out"});
        assert!(listed_candidates(&found).is_none());
    }

    #[test]
    fn listed_candidates_rejects_a_completeness_warning() {
        let found = json!({"files": [], "total": 0, "completeness_warning": "skipped"});
        assert!(listed_candidates(&found).is_none());
    }

    #[test]
    fn listed_candidates_rejects_a_result_without_a_files_array() {
        assert!(listed_candidates(&json!({"total": 0})).is_none());
        assert!(listed_candidates(&json!({"files": "x"})).is_none());
    }

    #[test]
    fn only_rust_and_python_callers_share_one_extension() {
        assert!(callers_share_one_extension("rust"));
        assert!(callers_share_one_extension("python"));
        for lang in ["typescript", "javascript", "kotlin", "java", "go"] {
            assert!(!callers_share_one_extension(lang), "{lang}");
        }
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
