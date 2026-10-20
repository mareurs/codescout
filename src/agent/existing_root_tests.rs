//! A root that is not an existing directory is REFUSED, by every path that would make it a
//! project, and the refusal creates NOTHING on disk.
//!
//! Before 2026-10-18 `Agent::ensure_resident` (the per-call `workspace=` pin), `Agent::activate`
//! and `Agent::new` each did `canonicalize(..).unwrap_or(raw)`: a missing path was kept as a
//! root and the first touch built a project there (`<root>/.codescout/{memories,
//! private-memories, usage.db, write.lock}` and a `.gitignore`). `activate_within_workspace`
//! did the same for a sub-project directory deleted after discovery.
//! docs/issues/2026-10-18-a-workspace-pin-naming-a-path-that-does-not-exist-creates-a-project-there.md
//!
//! These test the `Agent` entrances directly. The dispatch-level refusal (the one a tool call
//! meets first) is tested in `server/workspace_pin_tests.rs`; the two layers are separate guards,
//! so each has its own tests.

use super::*;
use tempfile::tempdir;

/// Every path under `root`, relative to it, sorted: "created nothing" is a claim about the tree.
fn tree_listing(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            out.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
            if entry.file_type().unwrap().is_dir() {
                walk(&path, root, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// The refusal must be a `RecoverableError` (so it routes to `isError: false`) that names the
/// path and says it is not an existing directory.
fn assert_refusal_names(err: &anyhow::Error, path: &Path, what: &str) {
    let recoverable = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .unwrap_or_else(|| panic!("must be a RecoverableError, got: {err:#}"));
    assert!(
        recoverable.message.contains(&path.display().to_string()),
        "must name {}; got: {}",
        path.display(),
        recoverable.message
    );
    // WHICH argument is refused: asserted per caller, so a swapped label is a red test.
    assert!(
        recoverable.message.starts_with(&format!("{what} '")),
        "must name the argument as `{what}`; got: {}",
        recoverable.message
    );
    assert!(
        recoverable.message.contains("not an existing directory"),
        "must say what is wrong; got: {}",
        recoverable.message
    );
    assert!(
        recoverable.hint().is_some_and(|h| h.contains("absolute")),
        "must say what to do; got: {:?}",
        recoverable.hint()
    );
}

async fn agent_at_home() -> (tempfile::TempDir, PathBuf, Agent) {
    let home = tempdir().unwrap();
    let home_root = std::fs::canonicalize(home.path()).unwrap();
    let agent = Agent::new(Some(home_root.clone())).await.unwrap();
    (home, home_root, agent)
}

#[tokio::test]
async fn ensure_resident_refuses_a_missing_directory_and_creates_nothing() {
    let (_home, _home_root, agent) = agent_at_home().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let missing = host_root.join("does-not-exist");

    let err = agent.ensure_resident(missing.clone()).await.unwrap_err();
    assert_refusal_names(&err, &missing, "workspace pin");
    assert_eq!(tree_listing(&host_root), Vec::<String>::new());

    // The pinned accessors reach it through `with_project_at`, which must propagate the same
    // refusal rather than swallow it into a default.
    let err = agent
        .with_project_at(Some(&missing), |p| Ok(p.root().to_path_buf()))
        .await
        .unwrap_err();
    assert_refusal_names(&err, &missing, "workspace pin");
    assert_eq!(tree_listing(&host_root), Vec::<String>::new());
    assert!(
        !agent.inner.read().await.workspaces.contains_key(&missing),
        "a refused pin must not be resident"
    );
}

#[tokio::test]
async fn ensure_resident_refuses_a_file_and_leaves_it_untouched() {
    let (_home, _home_root, agent) = agent_at_home().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let file = host_root.join("not-a-dir.txt");
    std::fs::write(&file, "keep").unwrap();

    let err = agent.ensure_resident(file.clone()).await.unwrap_err();
    assert_refusal_names(&err, &file, "workspace pin");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "keep");
    assert_eq!(tree_listing(&host_root), vec!["not-a-dir.txt".to_string()]);
}

#[tokio::test]
async fn ensure_resident_still_makes_an_existing_directory_resident() {
    let (_home, home_root, agent) = agent_at_home().await;
    let other = tempdir().unwrap();
    let other_root = std::fs::canonicalize(other.path()).unwrap();

    agent.ensure_resident(other_root.clone()).await.unwrap();

    assert!(agent
        .inner
        .read()
        .await
        .workspaces
        .contains_key(&other_root));
    assert_eq!(
        agent.project_root().await,
        Some(home_root),
        "a pin never moves the session default"
    );
}

#[tokio::test]
async fn activate_refuses_a_missing_directory_and_changes_nothing() {
    let (_home, home_root, agent) = agent_at_home().await;
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let missing = host_root.join("does-not-exist");

    let err = agent.activate(missing.clone(), None).await.unwrap_err();
    assert_refusal_names(&err, &missing, "project root");
    assert_eq!(tree_listing(&host_root), Vec::<String>::new());
    assert_eq!(
        agent.project_root().await,
        Some(home_root),
        "a refused activation must leave the session's project where it was"
    );
}

#[tokio::test]
async fn new_refuses_a_missing_directory_and_creates_nothing() {
    let host = tempdir().unwrap();
    let host_root = std::fs::canonicalize(host.path()).unwrap();
    let missing = host_root.join("does-not-exist");

    let err = Agent::new(Some(missing.clone()))
        .await
        .err()
        .expect("a missing --project must be refused");
    assert_refusal_names(&err, &missing, "project root");
    assert_eq!(tree_listing(&host_root), Vec::<String>::new());
}

/// A sub-project is discovered from a manifest on disk, so its directory exists at discovery;
/// activating it later builds a lock file there (`create_dir_all`). If the directory was deleted
/// in between, activation must refuse rather than recreate it.
#[tokio::test]
async fn activate_within_workspace_refuses_a_sub_project_deleted_after_discovery() {
    let home = tempdir().unwrap();
    let home_root = std::fs::canonicalize(home.path()).unwrap();
    let sub = home_root.join("packages").join("api");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(
        sub.join("package.json"),
        r#"{"name":"api","scripts":{"build":"tsc"}}"#,
    )
    .unwrap();
    let agent = Agent::new(Some(home_root.clone())).await.unwrap();
    std::fs::remove_dir_all(&sub).unwrap();

    let err = agent
        .activate_within_workspace("api", None)
        .await
        .unwrap_err();
    assert_refusal_names(&err, &sub, "sub-project root");
    assert!(
        !sub.exists(),
        "a refused sub-project activation must not recreate its directory"
    );
}

/// F3 (M3): residency must not outlive the directory. A workspace that is already resident but
/// whose directory has since been deleted is refused on the next call, not served from the
/// cached entry. Residency is checked AFTER the directory, so a stale entry cannot answer.
#[tokio::test]
async fn ensure_resident_refuses_a_resident_workspace_whose_directory_was_deleted() {
    let (_home, _home_root, agent) = agent_at_home().await;
    let other = tempdir().unwrap();
    let other_root = std::fs::canonicalize(other.path()).unwrap();
    agent.ensure_resident(other_root.clone()).await.unwrap();
    std::fs::remove_dir_all(&other_root).unwrap();

    let err = agent.ensure_resident(other_root.clone()).await.unwrap_err();
    assert_refusal_names(&err, &other_root, "workspace pin");
    assert!(
        !other_root.exists(),
        "a refused pin must not re-create the deleted directory"
    );
}

/// F7: a path that cannot be REACHED is not "not an existing directory". A mode-000 parent
/// makes `canonicalize` fail with a permission error, and the old message claimed the path did
/// not exist, which sends the caller looking for a typo.
#[cfg(unix)]
#[test]
fn a_path_behind_an_unreadable_parent_is_refused_as_unreachable_not_as_missing() {
    use std::os::unix::fs::PermissionsExt;
    let host = tempdir().unwrap();
    let locked = std::fs::canonicalize(host.path()).unwrap().join("locked");
    let inner = locked.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    struct Restore(PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
        }
    }
    let _restore = Restore(locked.clone());
    if std::fs::read_dir(&locked).is_ok() {
        eprintln!("mode 000 does not bind this process; skipping");
        return;
    }

    let err = existing_project_dir(inner.clone(), "workspace pin").unwrap_err();
    assert!(
        err.message.contains("permission denied")
            && err.message.contains(&inner.display().to_string()),
        "must say the path cannot be reached and name it; got: {}",
        err.message
    );
    assert!(
        !err.message.contains("not an existing directory"),
        "must not claim the path is missing; got: {}",
        err.message
    );

    // The genuinely missing path keeps its own wording.
    let missing = existing_project_dir(host.path().join("nope"), "workspace pin").unwrap_err();
    assert!(
        missing.message.contains("not an existing directory"),
        "got: {}",
        missing.message
    );
}

/// H1: the loader's TOML error quotes the offending source line (syntax) or value (type). Only
/// the digits of the position survive; a type error, which has no position, keeps nothing.
#[test]
fn a_toml_error_keeps_its_position_and_nothing_it_quotes() {
    let syntax = toml::from_str::<toml::Value>("SECRET_MARKER_QX7 hunter2").unwrap_err();
    let location =
        toml_location(&anyhow::Error::new(syntax)).expect("a syntax error has a line and a column");
    assert!(
        location.starts_with(" (line 1, column ") && location.ends_with(')'),
        "got: {location}"
    );
    assert!(!location.contains("SECRET"), "got: {location}");

    let table = toml::from_str::<toml::Value>("project = \"SECRET_MARKER_QX7\"").unwrap();
    let type_error = table
        .try_into::<crate::config::project::ProjectConfig>()
        .unwrap_err();
    assert!(
        type_error.to_string().contains("SECRET_MARKER_QX7"),
        "premise: the type error quotes the value"
    );
    assert_eq!(toml_location(&anyhow::Error::new(type_error)), None);
    assert_eq!(toml_location(&anyhow::anyhow!("not toml at all")), None);
}

/// The refusal for a pin: a refusal that already exists passes through untouched, a config
/// failure names the file that failed (the project's or the global one) with the matching
/// hint, and anything else gets the generic hint. The pin is quoted in every case.
#[test]
fn a_pin_refusal_is_passed_through_names_the_failed_file_or_gets_the_generic_hint() {
    let root = Path::new("/p");
    let original = crate::tools::RecoverableError::with_hint("original message", "original hint");
    let passed = pin_resolution_refusal(root, original.into());
    assert_eq!(passed.message, "original message");
    assert_eq!(passed.hint(), Some("original hint"));

    let project = pin_resolution_refusal(
        root,
        anyhow::Error::new(ConfigLoadFailed {
            path: PathBuf::from("/p/.codescout/project.toml"),
            global: false,
            location: Some(" (line 1, column 2)".into()),
        }),
    );
    assert_eq!(
        project.message,
        "workspace pin '/p' could not be resolved: project config \
         '/p/.codescout/project.toml' could not be loaded (line 1, column 2)"
    );
    assert!(project
        .hint()
        .unwrap()
        .contains("project's .codescout/project.toml"));

    let global = pin_resolution_refusal(
        root,
        anyhow::Error::new(ConfigLoadFailed {
            path: PathBuf::from("/home/u/.config/codescout/config.toml"),
            global: true,
            location: None,
        }),
    );
    assert!(global
        .message
        .contains("global config '/home/u/.config/codescout/config.toml' could not be loaded"));
    assert!(global.hint().unwrap().contains("global config"));
    assert!(
        !global.hint().unwrap().contains("project.toml"),
        "a global failure must not send the caller to the project's file"
    );

    let denied: anyhow::Error = std::io::Error::from(std::io::ErrorKind::PermissionDenied).into();
    let generic = pin_resolution_refusal(root, denied);
    assert!(generic
        .message
        .starts_with("workspace pin '/p' could not be resolved"));
    assert!(!generic.hint().unwrap().contains("project.toml"));
}

/// The loader is not asked to be the classifier: ANY error text, however it is worded, is
/// reported as a config failure when and only when `ConfigLoadFailed` is in the chain.
#[test]
fn only_the_marker_makes_a_config_failure_whatever_the_loader_says() {
    for wording in [
        "invalid configuration for /p/.codescout/project.toml",
        "project.toml exceeds 1 MiB limit (2097152 bytes)",
        "parsing /p/.codescout/project.toml: unexpected token",
    ] {
        assert!(
            config_load_failure(&anyhow::anyhow!(wording)).is_none(),
            "text alone must not classify: {wording}"
        );
    }
    let toml_error = toml::from_str::<toml::Value>("= =").unwrap_err();
    assert!(config_load_failure(&anyhow::Error::new(toml_error)).is_none());
    let marked = anyhow::Error::new(ConfigLoadFailed {
        path: PathBuf::from("/p/.codescout/project.toml"),
        global: false,
        location: None,
    })
    .context("while resolving the pin");
    assert!(
        config_load_failure(&marked).is_some(),
        "the marker is found under a context"
    );
}

/// L3: what is echoed is bounded and escaped. The boundary cases matter: exactly at the limit
/// is untouched, one byte over is cut, and a multibyte character is never split.
#[test]
fn echoed_text_is_escaped_and_cut_at_the_bound() {
    assert_eq!(bounded_for_message("/plain/path"), "/plain/path");
    assert_eq!(bounded_for_message("a\nb\u{1b}c"), "a\\u{a}b\\u{1b}c");
    assert_eq!(bounded_for_message("tab\there"), "tab\\u{9}here");

    let exactly = "a".repeat(512);
    assert_eq!(bounded_for_message(&exactly), exactly);
    let over = "a".repeat(513);
    let cut = bounded_for_message(&over);
    assert!(cut.starts_with(&"a".repeat(512)), "got: {cut}");
    assert!(cut.ends_with("(truncated, 513 bytes in all)"), "got: {cut}");

    let wide = "é".repeat(1000);
    let cut = bounded_for_message(&wide);
    assert!(cut.contains("(truncated, 2000 bytes in all)"), "got: {cut}");
    assert!(cut.len() < 512 + 40, "got {} bytes", cut.len());
    assert!(cut.starts_with("éé") && !cut.contains('\u{fffd}'));
}

/// M1, through the REAL loader (no hand-built error): an unparseable file, a file over the
/// 1 MiB limit and a list-limit breach each come back as `ConfigLoadFailed` for the project's
/// own file, with nothing from inside the file in what is shown.
#[test]
fn the_real_loader_failures_become_the_marker_and_quote_nothing() {
    let host = tempdir().unwrap();
    let root = std::fs::canonicalize(host.path()).unwrap();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    let file = root.join(".codescout/project.toml");
    let oversize = format!("# SECRET_MARKER_QX7\n{}", "#".repeat(1024 * 1024 + 1));
    for (label, contents) in [
        ("syntax", "SECRET_MARKER_QX7 hunter2".to_string()),
        ("type", "project = \"SECRET_MARKER_QX7\"\n".to_string()),
        ("oversize", oversize),
        (
            "list limit",
            "[project]\nname = \"x\"\n[security]\nextra_write_roots = [\"SECRET_MARKER_QX7\"]\n"
                .to_string(),
        ),
    ] {
        std::fs::write(&file, contents).unwrap();
        let err = load_project_config(&root).unwrap_err();
        let failure =
            config_load_failure(&err).unwrap_or_else(|| panic!("{label}: not the marker: {err:#}"));
        assert!(!failure.global, "{label}: the project's file failed");
        assert_eq!(failure.path, file, "{label}");
        let shown = format!("{err:#} | {failure}");
        assert!(
            !shown.contains("SECRET_MARKER_QX7"),
            "{label}: quoted: {shown}"
        );
        assert!(
            shown.contains("could not be loaded"),
            "{label}: got: {shown}"
        );
    }
}

/// A `project.toml` that is a FIFO or a link to a directory is not silently defaulted: the real
/// loader refuses it and `load_project_config` marks it `ConfigLoadFailed` for the project's own
/// file, which is what the pin refusal and the failed-reload notice are built from. On the old
/// loader the FIFO case never returned at all.
#[cfg(unix)]
#[test]
fn a_special_file_as_project_toml_becomes_the_marker_not_a_hang_or_a_default() {
    use crate::util::fs::special_file_fixtures::{make_fifo, within_deadline};
    for label in ["fifo", "link to a directory"] {
        let host = tempdir().unwrap();
        let root = std::fs::canonicalize(host.path()).unwrap();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        let file = root.join(".codescout/project.toml");
        if label == "fifo" {
            make_fifo(&file);
        } else {
            let real = root.join("a-directory");
            std::fs::create_dir(&real).unwrap();
            std::os::unix::fs::symlink(&real, &file).unwrap();
        }
        let r = root.clone();
        let err = within_deadline(label, move || load_project_config(&r)).unwrap_err();
        let failure =
            config_load_failure(&err).unwrap_or_else(|| panic!("{label}: not the marker: {err:#}"));
        assert!(!failure.global, "{label}: the project's file failed");
        assert_eq!(failure.path, file, "{label}");
        assert!(
            format!("{failure}").contains("could not be loaded"),
            "{label}: got: {failure}"
        );
    }
}

/// `workspace.toml` is repository content: a FIFO there (or a link to one) must not hold
/// activation, and a link to a regular file must still be read.
#[cfg(unix)]
#[test]
fn read_discover_settings_refuses_a_special_file_and_reads_a_linked_regular_one() {
    use crate::util::fs::special_file_fixtures::{make_fifo, within_deadline};
    let host = tempdir().unwrap();
    let root = std::fs::canonicalize(host.path()).unwrap();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    let ws = crate::config::workspace::workspace_config_path(&root);

    make_fifo(&ws);
    let r = root.clone();
    let got = within_deadline("read_discover_settings on a FIFO", move || {
        read_discover_settings(&r)
    });
    assert_eq!(got, None);
    std::fs::remove_file(&ws).unwrap();

    let real = root.join("shared-workspace.toml");
    std::fs::write(
        &real,
        "exclude_projects = [\"node_modules\"]\n[workspace]\nname = \"w\"\ndiscovery_max_depth = 5\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(&real, &ws).unwrap();
    assert_eq!(
        read_discover_settings(&root),
        Some((5, vec!["node_modules".to_string()]))
    );
}

/// L3 for the mapper: the pin, and the text of a non-config failure, are echoed bounded and
/// escaped, like every other echo of caller-supplied text.
#[test]
fn a_pin_refusal_echoes_the_pin_and_the_error_bounded_and_escaped() {
    let long_root = PathBuf::from(format!("/{}", "r".repeat(2000)));
    let config = pin_resolution_refusal(
        &long_root,
        anyhow::Error::new(ConfigLoadFailed {
            path: PathBuf::from("/p/.codescout/project.toml"),
            global: false,
            location: None,
        }),
    );
    assert!(
        config.message.len() < 700 && config.message.contains("(truncated, 2001 bytes in all)"),
        "the pin in a config refusal is capped; got {} bytes",
        config.message.len()
    );

    let rough_root = PathBuf::from("/a\nb");
    let generic = pin_resolution_refusal(
        &rough_root,
        anyhow::anyhow!("{}\u{1b}[31m", "e".repeat(2000)),
    );
    assert!(
        generic
            .message
            .starts_with("workspace pin '/a\\u{a}b' could not be resolved: "),
        "the pin is escaped; got: {}",
        generic.message
    );
    assert!(
        generic.message.len() < 1200 && !generic.message.contains('\u{1b}'),
        "the error text is capped and escaped; got {} bytes",
        generic.message.len()
    );
    assert!(generic.message.contains("(truncated, "));
}

/// L3: the working directory named in the relative-path note is bounded too. Runs in a child
/// process whose cwd is a deep directory, so the shared suite's cwd is never touched.
#[cfg(unix)]
#[test]
fn a_long_cwd_is_bounded_in_the_relative_path_note() {
    let base = tempdir().unwrap();
    let base_root = std::fs::canonicalize(base.path()).unwrap();
    crate::platform::test_child::run(
        module_path!(),
        "a_long_cwd_is_bounded_in_the_relative_path_note",
        &[],
        || {
            let mut deep = base_root.clone();
            for level in 0..4 {
                deep.push(format!("{level}{}", "d".repeat(200)));
            }
            std::fs::create_dir_all(&deep).unwrap();
            std::env::set_current_dir(&deep).unwrap();
            let err = existing_project_dir(PathBuf::from("missing"), "workspace pin").unwrap_err();
            assert!(
                err.message.len() < 700 && err.message.contains("(truncated, "),
                "the cwd in the note is capped; got {} bytes: {}",
                err.message.len(),
                err.message
            );
        },
    );
}
