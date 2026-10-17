//! Run a test body in a CHILD process with a chosen environment.
//!
//! For tests whose subject reads process-global state (`$HOME`) that cannot be injected, and
//! whose parent suite must not mutate it (`docs/conventions/test-env-isolation.md`): the test
//! re-executes the test binary on itself (`--exact`, one thread) with a marker variable, the
//! environment edits applied to the CHILD only, and the child runs `body`. In the shared suite
//! the marker is absent, so `body` never runs there and nothing is mutated.
//!
//! This file is the ONE place in `src/platform` allowed to mutate the process environment
//! (`tests/env_mutation_isolation.rs` allowlists this file, not the whole `platform` module): the
//! stale-`$HOME` test below changes its own `$HOME`, and only ever inside such a child. Compiled
//! for tests on unix only (`#[cfg(all(test, unix))]` at the `mod` declaration).

use super::{account_home_dir, resolved_home_dir};
use std::path::PathBuf;

const MARKER: &str = "CODESCOUT_TEST_CHILD";

/// `module` is `module_path!()` of the caller, `name` the caller's own function name; `envs`
/// are applied to the child (`Some` sets, `None` removes).
pub(crate) fn run(module: &str, name: &str, envs: &[(&str, Option<&str>)], body: impl FnOnce()) {
    // libtest names tests without the crate segment `module_path!()` starts with.
    let module = module.split_once("::").map_or(module, |(_, rest)| rest);
    let full = format!("{module}::{name}");
    if std::env::var_os(MARKER).is_some_and(|v| v == full.as_str()) {
        body();
        return;
    }
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args(["--exact", &full, "--test-threads=1"])
        .env(MARKER, &full);
    for (key, value) in envs {
        match value {
            Some(v) => cmd.env(key, v),
            None => cmd.env_remove(key),
        };
    }
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "child {full} failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("1 passed"),
        "the child must have run {full}, not filtered it out:\n{stdout}"
    );
}

/// The cache is for the ACCOUNT lookup only. A cache over `resolved_home_dir` as a whole would
/// keep answering with the `$HOME` of the first call.
///
/// Proving that takes a `$HOME` that changes inside one process, and mutating process env in the
/// shared test process is banned (`docs/conventions/test-env-isolation.md`). So the CHILD of
/// `run`, which runs nothing else, changes its own `$HOME`.
#[test]
fn resolved_home_dir_is_not_stale_after_home_changes() {
    run(
        module_path!(),
        "resolved_home_dir_is_not_stale_after_home_changes",
        &[],
        || {
            // Child: a single-purpose process, so no other thread reads `HOME`.
            let account = account_home_dir(); // fills the account cache first
            std::env::set_var("HOME", "/child-home-a");
            assert_eq!(resolved_home_dir(), Some(PathBuf::from("/child-home-a")));
            std::env::set_var("HOME", "/child-home-b");
            assert_eq!(
                resolved_home_dir(),
                Some(PathBuf::from("/child-home-b")),
                "a changed $HOME must be seen on the next call"
            );
            for unusable in ["", "relative/home"] {
                std::env::set_var("HOME", unusable);
                assert_eq!(resolved_home_dir(), account, "HOME={unusable:?}");
            }
            std::env::remove_var("HOME");
            assert_eq!(resolved_home_dir(), account, "HOME unset");
        },
    );
}
