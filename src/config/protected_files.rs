//! Refusing a tool write that would change the settings which decide what the tools may do.
//!
//! `<project>/.codescout/project.toml` sits under the always-writable project root, and the
//! global `config.toml` is merged under every project. Both carry a `[security]` table
//! (`profile`, `extra_write_roots`, `file_write_enabled`, `shell_command_mode`, ...), and the
//! server re-reads a config file when it changes. A write tool that could edit either file
//! could therefore raise its own privileges from the next call: a prompt-injected model writes
//! `profile = "root"` and every write gate is off.
//!
//! The guard sits in [`crate::util::fs::atomic_write`], the one function `create_file`,
//! `edit_file` (every grammar) and `edit_code` write through, so a write tool added later is
//! covered on the day it is added. It is **content-aware**, not a blanket path deny, because
//! the rest of the file is legitimately edited through those tools: the onboarding prompt's
//! embedding-model step tells the model to `edit_file` `.codescout/project.toml` and change
//! `[embeddings].model`. A write is refused only when the `[security]` table it produces
//! differs from the one on disk, or when the new content is not TOML and so cannot be shown
//! to leave it alone.
//!
//! **Scope, stated so it is not read as wider.** This is a property of the file-writing
//! tools. It does not stop `run_command` (a shell can write any file the user can, and is
//! gated by `shell_command_mode`), a human editing the file, or codescout's own `onboarding`
//! tool, which writes `project.toml` directly with `std::fs::write` and not through this
//! function. A `profile = "root"` profile is also unaffected: it already allows every write.

use std::ffi::OsStr;
use std::path::Path;

/// The top-level table this guard keeps out of a tool's reach.
const PROTECTED_TABLE: &str = "security";

/// Check a write of `new_content` to `path` against the ambient global config location.
///
/// `Ok` for every path that is not a codescout config file. `Err` carries the message the
/// caller reads, which names the one step that is theirs to take.
pub(crate) fn check_write(path: &Path, new_content: &str) -> Result<(), String> {
    check_write_with(
        path,
        new_content,
        crate::config::global::global_config_path().as_deref(),
    )
}

/// [`check_write`] with the global config path supplied, so a test can name it without
/// touching the process environment.
pub(crate) fn check_write_with(
    path: &Path,
    new_content: &str,
    global_config: Option<&Path>,
) -> Result<(), String> {
    if !is_project_toml(path) && !is_global_config(path, global_config) {
        return Ok(());
    }
    let new: toml::Table = toml::from_str(new_content).map_err(|e| {
        refusal(
            path,
            &format!(
                "the new content is not valid TOML ({e}), so it cannot be shown to leave \
                 [{PROTECTED_TABLE}] unchanged"
            ),
        )
    })?;
    if security_on_disk(path).as_ref() == new.get(PROTECTED_TABLE) {
        return Ok(());
    }
    Err(refusal(
        path,
        &format!("this write would change its [{PROTECTED_TABLE}] section"),
    ))
}

/// The `[security]` table of the file as it is now: `None` when the file is absent, is not
/// TOML, or has no such table. An unreadable "before" is the same as an empty one, so a
/// write cannot use a broken file to smuggle a table in.
fn security_on_disk(path: &Path) -> Option<toml::Value> {
    let text = std::fs::read_to_string(path).ok()?;
    let table: toml::Table = toml::from_str(&text).ok()?;
    table.get(PROTECTED_TABLE).cloned()
}

/// A file named `project.toml` directly inside a directory named `.codescout`, matched
/// ignoring ASCII case: on a case-insensitive filesystem another spelling names the same file,
/// and on a case-sensitive one denying it costs nothing.
fn is_project_toml(path: &Path) -> bool {
    named(path.file_name(), "project.toml")
        && named(path.parent().and_then(Path::file_name), ".codescout")
}

fn named(name: Option<&OsStr>, expected: &str) -> bool {
    name.and_then(OsStr::to_str)
        .is_some_and(|n| n.eq_ignore_ascii_case(expected))
}

/// `path` is the global config file: the same path, or the same file name in a directory
/// that resolves to the same place (the parent is canonicalized, as the write tools do).
fn is_global_config(path: &Path, global_config: Option<&Path>) -> bool {
    let Some(global) = global_config else {
        return false;
    };
    if path == global {
        return true;
    }
    if path.file_name() != global.file_name() {
        return false;
    }
    match (
        path.parent().and_then(|p| std::fs::canonicalize(p).ok()),
        global.parent().and_then(|p| std::fs::canonicalize(p).ok()),
    ) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn refusal(path: &Path, why: &str) -> String {
    format!(
        "write denied: '{}' is a codescout configuration file and {why}. [{PROTECTED_TABLE}] \
         decides what the tools may do (profile, write roots, file writes, shell mode), so the \
         write tools cannot change it: a session that could would be able to raise its own \
         privileges. Ask the user to make that edit in an editor; the server re-reads the file \
         on its next call. Edits to the rest of the file are allowed.",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proj(dir: &tempfile::TempDir) -> std::path::PathBuf {
        let cs = dir.path().join(".codescout");
        std::fs::create_dir_all(&cs).unwrap();
        cs.join("project.toml")
    }

    /// Names that are not config files are never touched, whatever they contain.
    #[test]
    fn only_the_config_files_are_guarded() {
        let dir = tempfile::tempdir().unwrap();
        let root = "[security]\nprofile = \"root\"\n";
        for p in [
            dir.path().join("project.toml"),
            dir.path().join(".codescout").join("notes.toml"),
            dir.path().join("docs").join("project.toml"),
            dir.path()
                .join(".codescout")
                .join("sub")
                .join("project.toml"),
        ] {
            assert_eq!(check_write_with(&p, root, None), Ok(()), "{}", p.display());
        }
    }

    #[test]
    fn project_toml_is_matched_ignoring_ascii_case() {
        let dir = tempfile::tempdir().unwrap();
        for p in [
            dir.path().join(".codescout").join("project.toml"),
            dir.path().join(".CodeScout").join("Project.TOML"),
        ] {
            assert!(
                check_write_with(&p, "[security]\nprofile = \"root\"\n", None).is_err(),
                "{}",
                p.display()
            );
        }
    }

    #[test]
    fn a_changed_added_or_removed_security_table_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let p = proj(&dir);
        std::fs::write(&p, "[security]\nshell_command_mode = \"warn\"\n").unwrap();
        // changed value
        assert!(check_write_with(
            &p,
            "[security]\nshell_command_mode = \"unrestricted\"\n",
            None
        )
        .is_err());
        // added key
        assert!(check_write_with(
            &p,
            "[security]\nshell_command_mode = \"warn\"\nprofile = \"root\"\n",
            None
        )
        .is_err());
        // removed table
        assert!(check_write_with(&p, "[project]\nname = \"t\"\n", None).is_err());
    }

    #[test]
    fn an_equal_security_table_is_allowed_whatever_else_changes() {
        let dir = tempfile::tempdir().unwrap();
        let p = proj(&dir);
        std::fs::write(
            &p,
            "[project]\nname = \"a\"\n\n[security]\nshell_command_mode = \"warn\"\n",
        )
        .unwrap();
        assert_eq!(
            check_write_with(
                &p,
                "# c\n[embeddings]\nmodel = \"m\"\n[project]\nname = \"b\"\n\n[security]\nshell_command_mode = \"warn\"\n",
                None
            ),
            Ok(())
        );
    }

    /// An existing file with no `[security]` table, edited without adding one: nothing
    /// protected changes, so the write is ordinary.
    #[test]
    fn editing_a_file_that_has_no_security_table_is_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let p = proj(&dir);
        std::fs::write(&p, "[project]\nname = \"a\"\n").unwrap();
        assert_eq!(
            check_write_with(
                &p,
                "[project]\nname = \"b\"\n\n[embeddings]\nmodel = \"m\"\n",
                None
            ),
            Ok(())
        );
    }

    /// No file yet: a create is the same question with an empty "before".
    #[test]
    fn creating_the_file_may_not_carry_a_security_table() {
        let dir = tempfile::tempdir().unwrap();
        let p = proj(&dir);
        assert!(!p.exists());
        assert!(check_write_with(&p, "[security]\nprofile = \"root\"\n", None).is_err());
        assert_eq!(
            check_write_with(&p, "[project]\nname = \"t\"\n", None),
            Ok(())
        );
    }

    /// Content that is not TOML cannot be shown to leave the table alone.
    #[test]
    fn content_that_is_not_toml_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let p = proj(&dir);
        let err = check_write_with(&p, "[security\nprofile = \"root\"\n", None).unwrap_err();
        assert!(err.contains("not valid TOML"), "{err}");
    }

    /// A broken file on disk is an empty "before": it cannot be used to smuggle a table in.
    #[test]
    fn a_broken_file_on_disk_is_an_empty_before() {
        let dir = tempfile::tempdir().unwrap();
        let p = proj(&dir);
        std::fs::write(&p, "[security\n").unwrap();
        assert!(check_write_with(&p, "[security]\nprofile = \"root\"\n", None).is_err());
        assert_eq!(
            check_write_with(&p, "[project]\nname = \"t\"\n", None),
            Ok(())
        );
    }

    /// The global config is guarded wherever the caller says it lives, including through a
    /// directory that is a link to it.
    #[test]
    fn the_global_config_is_guarded_by_exact_path_and_by_resolved_directory() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir_all(&real).unwrap();
        let global = real.join("config.toml");
        std::fs::write(&global, "[security]\nshell_command_mode = \"warn\"\n").unwrap();
        let change = "[security]\nshell_command_mode = \"unrestricted\"\n";

        assert!(check_write_with(&global, change, Some(&global)).is_err());
        // not the global config when none is named
        assert_eq!(check_write_with(&global, change, None), Ok(()));
        // another file in the same directory is not it
        assert_eq!(
            check_write_with(&real.join("other.toml"), change, Some(&global)),
            Ok(())
        );
        // the same file NAME in another directory is not it
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        assert_eq!(
            check_write_with(&elsewhere.join("config.toml"), change, Some(&global)),
            Ok(())
        );
        // a directory that cannot be resolved is not the global config's directory...
        assert_eq!(
            check_write_with(
                &dir.path().join("no-such-dir").join("config.toml"),
                change,
                Some(&global)
            ),
            Ok(())
        );
        // ...unless the path IS the one named, which needs no resolving
        let unresolvable = dir.path().join("no-such-global").join("config.toml");
        assert!(check_write_with(&unresolvable, change, Some(&unresolvable)).is_err());
        // the same directory reached through a link
        #[cfg(unix)]
        {
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(&real, &link).unwrap();
            assert!(check_write_with(&link.join("config.toml"), change, Some(&global)).is_err());
        }
    }
}
