//! Prevention guard: refuse artifact writes that would land a temp-dir-rooted
//! artifact in the real/shared (file-backed, outside-temp) catalog — the vector
//! that polluted the global catalog with /tmp probe rows.
//! See docs/issues/archive/2026-07-17-tmp-probe-artifacts-pollute-global-catalog.md.

use std::path::{Path, PathBuf};

const ALLOW_ENV: &str = "CODESCOUT_ALLOW_TEMP_WORKSPACE";

/// The process-environment inputs this guard needs, resolved **once, at the edge**.
///
/// Its whole reason for existing is that the decision must never re-read the environment
/// mid-call, so a test can *state* what counts as temp instead of inheriting the machine's.
/// `docs/conventions/test-env-isolation.md` forbids the alternative outright: `set_var` is
/// process-global and `#[serial]` only locks against tests that opt in, so a guard-plus-serial
/// shape narrows the race without closing it (option B, "NOT VIABLE"). This is option A —
/// resolve at the edge, pass the result inward — the same shape as `LibrarianEnv::from_env`.
///
/// The concrete failure that forced it: the wiring tests built their "outside-temp" catalog
/// under `std::env::current_dir()`, which silently becomes an *inside*-temp catalog when the
/// cwd is itself under the OS temp dir. `should_refuse` then correctly declined to fire and
/// `expect_err` panicked with a message blaming the guard — an unmet premise reported as a
/// guard defect. `docs/issues/archive/2026-08-30-temp-guard-tests-fail-from-a-tmp-checkout.md`.
#[derive(Clone, Debug)]
pub struct TempGuardEnv {
    /// What counts as "the OS temp dir" for this decision, canonicalized.
    pub temp_dir: PathBuf,
    /// Whether the caller explicitly opted out of the guard.
    pub opted_in: bool,
}

impl TempGuardEnv {
    /// The only place this module reads the process environment.
    pub fn from_env() -> Self {
        // Value-based opt-in: only an explicit truthy value disables the guard, so a
        // stray `CODESCOUT_ALLOW_TEMP_WORKSPACE=0` can't silently defeat prevention.
        let opted_in = std::env::var(ALLOW_ENV)
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let temp_dir =
            std::fs::canonicalize(std::env::temp_dir()).unwrap_or_else(|_| std::env::temp_dir());
        Self { temp_dir, opted_in }
    }
}

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
    env: &TempGuardEnv,
) -> anyhow::Result<()> {
    // Fail-open: if `root` cannot be canonicalized (e.g. it does not exist), fall
    // back to the raw path. create/reindex roots exist at guard time, so this only
    // risks a false-ALLOW on a symlinked temp dir for a nonexistent root — never a
    // false-REFUSE.
    let root_c = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let catalog_db = crate::librarian::catalog::catalog_db_path(conn)
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p));
    if should_refuse(&root_c, catalog_db.as_deref(), &env.temp_dir, env.opted_in) {
        return Err(crate::librarian::tools::RecoverableError::with_hint(
            format!(
                "refusing to write an artifact rooted under the system temp dir ({}) into the \
                 shared persistent catalog — this is how probe/test runs pollute the catalog",
                env.temp_dir.display()
            ),
            format!(
                "Use an isolated catalog (under the temp dir, or in-memory) for tests, or set \
                 {ALLOW_ENV}=1 if this scratch workspace is intentional."
            ),
        ));
    }
    Ok(())
}

/// A synthetic temp root for tests that must exercise a REFUSAL.
///
/// Returns `(scratch, env, inside, outside)` where `inside` is under the guard's
/// `temp_dir` and `outside` is not — **both physically under the OS temp dir**. That
/// inversion is the entire point: what makes one "inside" and the other "outside" is the
/// injected `temp_dir`, not where the suite happens to be running from.
///
/// A refusal needs a catalog the guard classifies as outside-temp, and **no directory is
/// guaranteed to sit outside `std::env::temp_dir()` on disk**, so it cannot be built by
/// choosing a location. These tests used to derive one from `std::env::current_dir()`,
/// which silently becomes an *inside*-temp catalog whenever the cwd is under the OS temp
/// dir; the guard then correctly declined and `expect_err` panicked with a message blaming
/// the guard. Measured 2026-08-31 — same binary, same source, cwd the only variable: 3
/// failed from a `/tmp` cwd, 3 passed from the checkout.
/// `docs/issues/archive/2026-08-30-temp-guard-tests-fail-from-a-tmp-checkout.md`.
///
/// Keep `scratch` alive for the duration of the test; dropping it removes both directories.
/// Both paths are canonicalized because the guard canonicalizes what it compares — on a host
/// whose temp dir is a symlink, an uncanonicalized prefix would not match and the fixture
/// would silently stop discriminating.
#[cfg(test)]
pub(crate) fn synthetic_temp() -> (tempfile::TempDir, TempGuardEnv, PathBuf, PathBuf) {
    let scratch = tempfile::TempDir::new().unwrap();
    let root = std::fs::canonicalize(scratch.path()).unwrap();
    let temp_dir = root.join("temp");
    let outside = root.join("real");
    std::fs::create_dir_all(&temp_dir).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let env = TempGuardEnv {
        temp_dir: temp_dir.clone(),
        opted_in: false,
    };
    (scratch, env, temp_dir, outside)
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
        let (_scratch, env, inside, _outside) = synthetic_temp();
        let cat = Catalog::open(&inside.join("librarian.db")).unwrap();
        let ws = inside.join("workspace");
        std::fs::create_dir_all(&ws).unwrap();
        assert!(guard_temp_workspace_write(&ws, &cat.conn, &env).is_ok());
    }

    #[test]
    fn wrapper_allows_in_memory_catalog_with_temp_workspace() {
        let (_scratch, env, inside, _outside) = synthetic_temp();
        let cat = Catalog::open_in_memory().unwrap();
        assert!(guard_temp_workspace_write(&inside, &cat.conn, &env).is_ok());
    }

    #[test]
    fn wrapper_refuses_temp_workspace_into_real_outside_temp_catalog() {
        // The real pollution shape: catalog outside the guard's temp root, workspace
        // under it. Both live under the OS temp dir on disk — see `synthetic_temp` for
        // why that is deliberate rather than sloppy.
        let (_scratch, env, inside, outside) = synthetic_temp();
        let cat = Catalog::open(&outside.join("catalog.db")).unwrap();
        let ws = inside.join("ws");
        std::fs::create_dir_all(&ws).unwrap();

        let err = guard_temp_workspace_write(&ws, &cat.conn, &env)
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

    /// Non-vacuity for the three refusal tests above: the SAME fixture is allowed once the
    /// caller opts in.
    ///
    /// Without this, a fixture that quietly stopped satisfying the refusal precondition —
    /// the failure this whole change exists to fix — would turn every refusal test green
    /// again, and green is what they report when they are working. Flipping one input and
    /// requiring the verdict to flip with it is what pins the refusal to the guard's
    /// decision rather than to an accident of the fixture. Deliberately a fixture flip and
    /// not a source mutation: the tree is shared with concurrent sessions, and a transient
    /// edit to `should_refuse` would show up in their test runs as an unexplained red.
    #[test]
    fn the_same_fixture_is_allowed_once_the_caller_opts_in() {
        let (_scratch, env, inside, outside) = synthetic_temp();
        let cat = Catalog::open(&outside.join("catalog.db")).unwrap();
        let ws = inside.join("ws");
        std::fs::create_dir_all(&ws).unwrap();

        let opted_in = TempGuardEnv {
            opted_in: true,
            ..env.clone()
        };
        assert!(
            guard_temp_workspace_write(&ws, &cat.conn, &opted_in).is_ok(),
            "opting in must allow the very fixture the refusal tests reject"
        );
        // And the un-opted-in verdict on the identical fixture is still a refusal, so the
        // assertion above cannot be passing because the fixture went inert.
        assert!(
            guard_temp_workspace_write(&ws, &cat.conn, &env).is_err(),
            "the same fixture without opt-in must still be refused"
        );
    }
}
