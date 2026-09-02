//! Keep a catalog row in step with a direct frontmatter write.
//!
//! `edit_markdown(path, frontmatter={set: {status: "fixed"}})` — the call
//! `get_guide("tracker-conventions")` and `edit_markdown`'s own `long_docs` row 3d both
//! recommend for flipping a bug's status — writes the file and never touches the catalog.
//! The row keeps its pre-edit `status` indefinitely, so
//! `doc(find, kind="bug", status=…)`, the triage query CLAUDE.md and the activation
//! bootstrap both prescribe, reports a value the file contradicts.
//!
//! The divergence is silent by construction, and the population is the one the guard lets
//! through on purpose. `librarian_guard` refuses augmented artifacts, stamped ids and
//! declared ledgers; a plain bug file is none of those, so it passes — correctly, per the
//! guard's own pinned `a_catalogued_but_unaugmented_file_stays_directly_editable`. Measured
//! 2026-08-30: **4 of 19** live files under `docs/issues/` carry a stamped `id:`, so 15 are
//! editable and every one of them can desync. See
//! `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md` and
//! `open-issue-work-queue:BL-48`.
//!
//! ## Why a hook rather than a call
//!
//! `edit_markdown` is a core tool; the catalog lives behind `#[cfg(feature = "librarian")]`.
//! A direct call would compile here and fail CI's `--no-default-features` lane, which is the
//! failure mode `CLAUDE.md` names explicitly. So this mirrors
//! [`super::librarian_guard`]'s oracle: a trait, a process-wide slot the librarian runtime
//! fills at startup, and a no-op when unset. The two modules are siblings on purpose —
//! same resolution, same lifetime, opposite directions (the guard reads the catalog to
//! refuse a write; this writes the catalog after one).

/// Writes a file's catalog row back into step with its on-disk frontmatter.
///
/// Implemented by the librarian runtime. **Must never create a row**: a path with no
/// catalog entry is ordinary markdown, and inventing an artifact for it would turn every
/// `edit_markdown` on a stray `.md` into a catalog write.
pub trait CatalogFrontmatterSync: Send + Sync {
    /// Re-read `abs_path`'s frontmatter and update the indexed columns of its existing
    /// row. Returns `true` if a row was found and updated, `false` if the path is not in
    /// the catalog.
    fn sync_frontmatter(&self, abs_path: &std::path::Path) -> bool;
}

/// The slot a hook lives in. Named so a test can own one and exercise the install
/// semantics without touching the process-wide [`HOOK`].
type HookSlot = std::sync::RwLock<Option<std::sync::Arc<dyn CatalogFrontmatterSync>>>;

/// Last-writer-wins, for the reason `librarian_guard::ORACLE` documents: a test binary
/// builds many servers, and a first-writer-wins `OnceLock` would pin whichever ran first
/// and make behaviour depend on test ordering (`bug-fix-session-log:F-51`).
static HOOK: HookSlot = std::sync::RwLock::new(None);

/// Install the process-wide hook. Called when the librarian runtime is built. Left unset
/// — tests, `--no-default-features` — the sync degrades to a no-op rather than failing.
pub fn install_catalog_sync(hook: std::sync::Arc<dyn CatalogFrontmatterSync>) {
    install_into(&HOOK, hook);
}

/// Write a hook into `slot`. Split out so the replacement semantics are testable against a
/// caller-owned slot; no test mutates the process-wide [`HOOK`], so no test perturbs another.
fn install_into(slot: &HookSlot, hook: std::sync::Arc<dyn CatalogFrontmatterSync>) {
    if let Ok(mut current) = slot.write() {
        *current = Some(hook);
    }
}

/// Clone the `Arc` out and drop the read lock before the caller uses it. Load-bearing for
/// the same reason as the guard's: `sync_frontmatter` takes the catalog lock, and holding
/// this one across that call would nest two locks for no reason.
fn read_from(slot: &HookSlot) -> Option<std::sync::Arc<dyn CatalogFrontmatterSync>> {
    slot.read().ok().and_then(|current| current.clone())
}

/// The testable core: same decision, hook passed explicitly.
///
/// Returns `true` only when a hook was installed AND it found a row to update — so
/// `false` covers both "no librarian" and "not an artifact", which callers treat
/// identically.
fn sync_with(hook: Option<&dyn CatalogFrontmatterSync>, abs_path: &std::path::Path) -> bool {
    match hook {
        Some(hook) => hook.sync_frontmatter(abs_path),
        None => false,
    }
}

/// Bring `abs_path`'s catalog row back into step after a direct frontmatter write.
///
/// Call only when the frontmatter actually changed — a body-only edit cannot move an
/// indexed column, and a write per body edit would be pure cost.
///
/// Never fails: a desynced row is a bug, but refusing the edit that caused it would be a
/// worse one, and the write has already landed by the time this runs.
pub fn sync_after_frontmatter_write(abs_path: &std::path::Path) -> bool {
    let hook = read_from(&HOOK);
    sync_with(hook.as_deref(), abs_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    /// Records every path it is asked about, and reports whether each was "in the catalog".
    struct Recording {
        seen: Mutex<Vec<PathBuf>>,
        found: bool,
    }

    impl Recording {
        fn new(found: bool) -> Self {
            Self {
                seen: Mutex::new(Vec::new()),
                found,
            }
        }
        fn seen(&self) -> Vec<PathBuf> {
            self.seen.lock().unwrap().clone()
        }
    }

    impl CatalogFrontmatterSync for Recording {
        fn sync_frontmatter(&self, abs_path: &Path) -> bool {
            self.seen.lock().unwrap().push(abs_path.to_path_buf());
            self.found
        }
    }

    /// The whole point of the module: an installed hook is actually asked, and asked about
    /// the path that was written.
    #[test]
    fn an_installed_hook_is_called_with_the_written_path() {
        let hook = Recording::new(true);
        let path = Path::new("/tmp/does-not-need-to-exist/bug.md");

        assert!(
            sync_with(Some(&hook), path),
            "a hook reporting a row was updated must surface as true"
        );
        assert_eq!(
            hook.seen(),
            vec![path.to_path_buf()],
            "the hook must be asked about exactly the path that was written, once"
        );
    }

    /// `--no-default-features`, and every test binary that never builds a librarian
    /// runtime. A missing hook is the normal case there, not an error.
    #[test]
    fn no_installed_hook_is_a_silent_no_op() {
        assert!(
            !sync_with(None, Path::new("/tmp/whatever.md")),
            "with no hook installed the sync must report false and do nothing"
        );
    }

    /// "Not in the catalog" and "no librarian" are the same answer to the caller, but they
    /// are different journeys — this pins that an ordinary markdown file does NOT get a row
    /// invented for it, which is the one way this module could do real damage.
    #[test]
    fn a_path_with_no_catalog_row_reports_false_without_creating_one() {
        let hook = Recording::new(false);
        let path = Path::new("/tmp/ordinary-notes.md");

        assert!(
            !sync_with(Some(&hook), path),
            "a path the catalog does not know must report false"
        );
        assert_eq!(
            hook.seen(),
            vec![path.to_path_buf()],
            "the hook is still consulted — it is the only thing that can answer the \
             question, and answering it is not the same as creating a row"
        );
    }

    /// Mirrors `librarian_guard`'s own install test, and for the same measured reason:
    /// a first-writer-wins slot made guard behaviour depend on test ordering (F-51).
    #[test]
    fn installing_twice_replaces_rather_than_discarding() {
        let slot: HookSlot = std::sync::RwLock::new(None);
        let first: Arc<dyn CatalogFrontmatterSync> = Arc::new(Recording::new(true));
        let second: Arc<dyn CatalogFrontmatterSync> = Arc::new(Recording::new(true));

        install_into(&slot, Arc::clone(&first));
        assert!(
            Arc::ptr_eq(&read_from(&slot).expect("installed"), &first),
            "the first install must be readable"
        );

        install_into(&slot, Arc::clone(&second));
        assert!(
            Arc::ptr_eq(&read_from(&slot).expect("still installed"), &second),
            "a later install must REPLACE the earlier one, not be discarded"
        );
    }
}
