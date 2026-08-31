//! Repo-root `.gitignore` compilation, shared by the consumers that need to ask
//! "does this project declare this path generated-or-machine-local?"
//!
//! Two callers today, for the same underlying reason and with different remedies:
//!
//! - `audit_doc_refs` caps a *missing* path's severity — an ignored path is
//!   expected absent in a clean checkout, so its absence carries no drift signal.
//! - `memory::anchors` refuses to *record* one — an anchor sidecar is tracked in
//!   git, so hashing an ignored file stores a fact about one machine inside a file
//!   that travels to all of them.

use std::path::Path;

/// Compile the repo-root `.gitignore`.
///
/// Only the root file is loaded — not nested `.gitignore`s and not the user's
/// global excludes. Callers use this to recognise paths *this repo* declares
/// generated-or-local, and a nested rule reaching such a path is rare enough not
/// to justify the extra walk. Any failure returns `None`, which disables the
/// caller's check rather than failing its run: a missing or malformed
/// `.gitignore` should cost precision, never the call.
pub fn build_root_gitignore(repo_root: &Path) -> Option<ignore::gitignore::Gitignore> {
    let path = repo_root.join(".gitignore");
    if !path.exists() {
        return None;
    }
    let mut builder = ignore::gitignore::GitignoreBuilder::new(repo_root);
    // `add` reports a parse error per file rather than returning Result; one bad
    // line should not discard the rules that did parse.
    let _ = builder.add(&path);
    builder.build().ok()
}

/// Whether `rel_path` is declared machine-local by the compiled matcher.
///
/// Uses `matched_path_or_any_parents`, not `matched`: a directory rule
/// (`.codescout/embeddings/`) must cover the files beneath it, and `matched`
/// tests only the path handed to it — so it answers "not ignored" for a file
/// inside an ignored directory. `None` (no `.gitignore`) means nothing is
/// declared local, so nothing matches.
pub fn is_machine_local(matcher: Option<&ignore::gitignore::Gitignore>, rel_path: &str) -> bool {
    matcher.is_some_and(|m| {
        m.matched_path_or_any_parents(Path::new(rel_path), false)
            .is_ignore()
    })
}
