//! Prevention guard: refuse artifact writes that would land a temp-dir-rooted
//! artifact in the real/shared (file-backed, outside-temp) catalog — the vector
//! that polluted the global catalog with /tmp probe rows.
//! See docs/issues/2026-07-17-tmp-probe-artifacts-pollute-global-catalog.md.

use std::path::Path;

const ALLOW_ENV: &str = "CODESCOUT_ALLOW_TEMP_WORKSPACE";

/// Pure decision: refuse iff the workspace `root` is under `temp_dir`, the
/// catalog is the real one (file-backed AND its file is outside `temp_dir`), and
/// the caller has not opted in. All inputs are pre-resolved so this is testable
/// with fabricated absolute paths (no filesystem access).
fn should_refuse(root: &Path, catalog_db: Option<&Path>, temp_dir: &Path, opted_in: bool) -> bool {
    if opted_in {
        return false;
    }
    let under_temp = |p: &Path| p.starts_with(temp_dir);
    let catalog_is_real = catalog_db.is_some_and(|c| !under_temp(c));
    under_temp(root) && catalog_is_real
}

/// Refuse a write whose workspace `root` is under the OS temp dir when the
/// catalog is the real/shared one. Canonicalizes the real inputs, then defers to
/// [`should_refuse`].
pub(crate) fn guard_temp_workspace_write(
    root: &Path,
    conn: &rusqlite::Connection,
) -> anyhow::Result<()> {
    // Value-based opt-in: only an explicit truthy value disables the guard, so a
    // stray `CODESCOUT_ALLOW_TEMP_WORKSPACE=0` can't silently defeat prevention.
    let opted_in = std::env::var(ALLOW_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let temp = std::fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
    // Fail-open: if `root` cannot be canonicalized (e.g. it does not exist), fall
    // back to the raw path. create/reindex roots exist at guard time, so this only
    // risks a false-ALLOW on a symlinked temp dir for a nonexistent root — never a
    // false-REFUSE.
    let root_c = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let catalog_db = crate::librarian::catalog::catalog_db_path(conn)
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p));
    if should_refuse(&root_c, catalog_db.as_deref(), &temp, opted_in) {
        return Err(crate::librarian::tools::RecoverableError::with_hint(
            format!(
                "refusing to write an artifact rooted under the system temp dir ({}) into the \
                 shared persistent catalog — this is how probe/test runs pollute the catalog",
                temp.display()
            ),
            format!(
                "Use an isolated catalog (under the temp dir, or in-memory) for tests, or set \
                 {ALLOW_ENV}=1 if this scratch workspace is intentional."
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use std::path::Path;

    #[test]
    fn should_refuse_temp_root_into_real_catalog() {
        assert!(should_refuse(
            Path::new("/tmp/scratch"),
            Some(Path::new("/home/u/.local/share/librarian/catalog.db")),
            Path::new("/tmp"),
            false,
        ));
    }

    #[test]
    fn should_allow_when_catalog_is_also_under_temp() {
        // The server/integration-test shape: file-backed catalog under the
        // test's own TempDir. Must be allowed.
        assert!(!should_refuse(
            Path::new("/tmp/ws"),
            Some(Path::new("/tmp/testrun/librarian.db")),
            Path::new("/tmp"),
            false,
        ));
    }

    #[test]
    fn should_allow_in_memory_catalog() {
        assert!(!should_refuse(
            Path::new("/tmp/ws"),
            None,
            Path::new("/tmp"),
            false
        ));
    }

    #[test]
    fn should_allow_non_temp_root() {
        assert!(!should_refuse(
            Path::new("/home/u/proj"),
            Some(Path::new("/home/u/.local/share/librarian/catalog.db")),
            Path::new("/tmp"),
            false,
        ));
    }

    #[test]
    fn should_allow_when_opted_in() {
        assert!(!should_refuse(
            Path::new("/tmp/ws"),
            Some(Path::new("/home/u/.local/share/librarian/catalog.db")),
            Path::new("/tmp"),
            true,
        ));
    }

    #[test]
    fn wrapper_allows_temp_workspace_with_temp_catalog() {
        // A file-backed catalog under temp (the test shape) must be allowed by
        // the real wrapper, proving it wires the probes correctly.
        let dir = tempfile::tempdir().unwrap(); // under the OS temp dir
        let cat = Catalog::open(&dir.path().join("librarian.db")).unwrap();
        let ws = dir.path().join("workspace");
        std::fs::create_dir_all(&ws).unwrap();
        assert!(guard_temp_workspace_write(&ws, &cat.conn).is_ok());
    }

    #[test]
    fn wrapper_allows_in_memory_catalog_with_temp_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        assert!(guard_temp_workspace_write(dir.path(), &cat.conn).is_ok());
    }

    #[test]
    fn wrapper_refuses_temp_workspace_into_real_outside_temp_catalog() {
        // Catalog OUTSIDE the OS temp dir (a temp dir under the repo cwd, auto-cleaned);
        // workspace UNDER the OS temp dir. This is the real pollution shape and the only
        // way to build an outside-temp catalog in a test without leaking files.
        // (Assumes the repo checkout is not itself under the OS temp dir, which holds here.)
        let outside = tempfile::TempDir::new_in(std::env::current_dir().unwrap()).unwrap();
        let cat = Catalog::open(&outside.path().join("catalog.db")).unwrap();
        let ws = tempfile::TempDir::new().unwrap(); // under the OS temp dir
        let err = guard_temp_workspace_write(ws.path(), &cat.conn)
            .expect_err("temp workspace + real (outside-temp) catalog must be refused");
        assert!(
            err.downcast_ref::<crate::librarian::tools::RecoverableError>()
                .is_some(),
            "refusal must be a librarian RecoverableError (routes to isError:false): {err}"
        );
        assert!(
            err.to_string().contains("temp dir"),
            "unexpected error: {err}"
        );
    }
}
