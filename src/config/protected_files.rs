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
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The top-level table this guard keeps out of a tool's reach.
const PROTECTED_TABLE: &str = "security";

/// Every project root whose config has been loaded in this process.
///
/// **Why a process-wide set and not agent state.** The guard sits in `util::fs::atomic_write`,
/// a free function every write tool shares and which holds no `Agent`; threading one through
/// would change the signature of the funnel the guard exists to sit in. The set only ever
/// GROWS and only widens what is protected, so sharing it between concurrent servers (and
/// between tests) cannot loosen anything: at worst a directory that was once loaded keeps its
/// `project.toml` guarded after the project is gone. Filled by
/// `ProjectConfig::load_with_global_base`, which every project's config goes through, so a
/// project cannot become resident without its root arriving here.
static PROJECT_ROOTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Record `root` as a project whose `.codescout/project.toml` must be recognised by identity.
pub(crate) fn register_project_root(root: &Path) {
    let mut roots = PROJECT_ROOTS.lock().unwrap_or_else(|e| e.into_inner());
    if !roots.iter().any(|r| r == root) {
        roots.push(root.to_path_buf());
    }
}

/// Check a write of `new_content` to `path` against the ambient global config location and the
/// project roots loaded so far.
///
/// `Ok` for every path that is not a codescout config file. `Err` carries the message the
/// caller reads, which names the one step that is theirs to take.
pub(crate) fn check_write(path: &Path, new_content: &str) -> Result<(), String> {
    let roots = PROJECT_ROOTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    check_write_with(
        path,
        new_content,
        crate::config::global::global_config_path().as_deref(),
        &roots,
    )
}

/// [`check_write`] with the global config path and the project roots supplied, so a test can
/// name them without touching the process environment.
pub(crate) fn check_write_with(
    path: &Path,
    new_content: &str,
    global_config: Option<&Path>,
    project_roots: &[PathBuf],
) -> Result<(), String> {
    if !is_project_toml(path)
        && !is_global_config(path, global_config)
        && !is_a_loaded_projects_config(path, project_roots)
    {
        return Ok(());
    }
    let new: toml::Table = toml::from_str(new_content).map_err(|e| {
        refusal(
            path,
            &format!(
                "the new content is not valid TOML ({}), so it cannot be shown to leave \
                 [{PROTECTED_TABLE}] unchanged",
                crate::config::project::describe_toml_error(Some(new_content), &e)
            ),
        )
    })?;
    let before = security_on_disk(path).map_err(|why| {
        refusal(
            path,
            &format!(
                "the file on disk cannot be read as it is ({why}), so it cannot be shown to \
                 leave [{PROTECTED_TABLE}] unchanged"
            ),
        )
    })?;
    if before.as_ref() == new.get(PROTECTED_TABLE) {
        return Ok(());
    }
    Err(refusal(
        path,
        &format!("this write would change its [{PROTECTED_TABLE}] section"),
    ))
}

/// The `[security]` table of the file as it is now.
///
/// `Ok(None)` when the file is absent (also a dangling link), is not TOML or not UTF-8, or has
/// no such table: an absent or broken "before" is the same as an empty one, so a write cannot
/// use a broken file to smuggle a table in. `Err(reason)` when a file IS there and cannot be
/// read as it is: not a regular file, over [`crate::util::fs::MAX_CONFIG_FILE_BYTES`], or one
/// the OS will not open. That is NOT an empty "before". The loader refuses such a file too, so
/// its `[security]` is unknown to this process, and treating it as empty let a write that
/// drops the table pass: pad the file past the cap (allowed, the table is unchanged), then
/// write a small one without the table (allowed, `None == None`).
fn security_on_disk(path: &Path) -> Result<Option<toml::Value>, String> {
    use crate::util::fs::BoundedReadError;
    let text = match crate::util::fs::read_regular_config_text(path) {
        Ok(text) => text,
        Err(e) if e.is_not_found() => return Ok(None),
        Err(BoundedReadError::Io(e)) if e.kind() == std::io::ErrorKind::InvalidData => {
            return Ok(None);
        }
        Err(e) => return Err(e.to_string()),
    };
    let Ok(table) = toml::from_str::<toml::Table>(&text) else {
        return Ok(None);
    };
    Ok(table.get(PROTECTED_TABLE).cloned())
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

/// `path` is the global config file, by identity (`same_file`: the same path, or any path that
/// resolves to it).
fn is_global_config(path: &Path, global_config: Option<&Path>) -> bool {
    global_config.is_some_and(|global| same_file(path, global))
}

/// `path` is, by identity, the `.codescout/project.toml` of a project loaded in this process.
///
/// The name rule (`is_project_toml`) sees only a spelling, and the write tools hand the guard a
/// path whose parent is already resolved. So `.codescout` as a link to a directory with another
/// name (`cfg-real/project.toml`) never matched it, nor did the file a `project.toml` LINK points
/// at, written under its own name. Both resolve to the same file as
/// `<root>/.codescout/project.toml`, which is what this compares.
fn is_a_loaded_projects_config(path: &Path, roots: &[PathBuf]) -> bool {
    // `path` is resolved ONCE, not once per root: this runs on every `atomic_write`, and a
    // long-lived server holds one root per project it ever loaded (measured: the per-root
    // resolve cost 1.4 ms at 100 roots and 15 ms at 1000 for an existing file).
    let target = resolve(path);
    let target_id = file_id(&target);
    roots.iter().any(|root| {
        let config = root.join(".codescout").join("project.toml");
        match (target_id, file_id(&config)) {
            // Both exist: the same file iff the same (device, inode). One `stat` per root,
            // and it also recognises a hard link, which comparing paths cannot.
            (Some(a), Some(b)) => a == b,
            // Exactly one exists, so they cannot be the same file (a dangling link is not an
            // existing file either).
            (Some(_), None) | (None, Some(_)) => false,
            // Neither exists (a create, a dangling link): only the resolved paths can say.
            (None, None) => resolve(&config) == target,
        }
    })
}

/// `(device, inode)` of the file `path` names (links followed), when it exists. `None` where
/// there is no such identity (not unix) or no such file.
#[cfg(unix)]
fn file_id(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

#[cfg(not(unix))]
fn file_id(_path: &Path) -> Option<(u64, u64)> {
    None
}

/// Whether `a` and `b` name the same file once every link is followed, for files that may not
/// exist yet (resolved through the deepest ancestor that does).
fn same_file(a: &Path, b: &Path) -> bool {
    resolve(a) == resolve(b)
}

/// How many links `resolve` follows before it gives up (the kernel's own limit is 40).
// cap-class: NOT_A_CAP — a loop guard on following links; hitting it returns the path as it stands, no result is truncated
const MAX_LINK_HOPS: usize = 40;

/// `path` with every link followed, the remainder appended as written, **including a link whose
/// target does not exist** (singly or in a chain). `canonicalize` fails on such a link exactly as
/// it does for a missing file, so a walk-up alone leaves the link's own name in the result and a
/// write to its missing TARGET never matches the link: `project.toml -> ../a.toml` with no
/// `a.toml` yet, then `create_file(a.toml)`. A leaf link IS followed (the file it names is the
/// one a write would change). After `MAX_LINK_HOPS` the path as it then stands is returned: a
/// loop cannot spin this, and it names no real file either.
fn resolve(path: &Path) -> PathBuf {
    let mut cur = path.to_path_buf();
    for _ in 0..MAX_LINK_HOPS {
        let (ancestor, below) = deepest_existing_ancestor(&cur);
        // `below` is deepest-first; its LAST entry is the component right under `ancestor`,
        // the first one that could not be resolved.
        let Some(first_missing) = below.last() else {
            return ancestor;
        };
        let link_at = ancestor.join(first_missing);
        let Ok(target) = std::fs::read_link(&link_at) else {
            // Not a link: a genuinely missing component. Nothing more to follow.
            return below.iter().rev().fold(ancestor, |acc, c| acc.join(c));
        };
        // A relative target is relative to the directory holding the link.
        let mut next = ancestor.join(target);
        for c in below[..below.len() - 1].iter().rev() {
            next.push(c);
        }
        cur = next;
    }
    cur
}

/// The deepest ancestor of `path` that `canonicalize` accepts (canonical), and the components
/// below it, deepest first.
fn deepest_existing_ancestor(path: &Path) -> (PathBuf, Vec<std::ffi::OsString>) {
    let mut below: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = path;
    loop {
        if let Ok(canon) = std::fs::canonicalize(cur) {
            return (canon, below);
        }
        let (Some(up), Some(name)) = (cur.parent(), cur.file_name()) else {
            // Nothing resolves (a relative path with no existing ancestor): hand it back whole.
            return (path.to_path_buf(), Vec::new());
        };
        below.push(name.to_os_string());
        cur = up;
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
            assert_eq!(
                check_write_with(&p, root, None, &[]),
                Ok(()),
                "{}",
                p.display()
            );
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
                check_write_with(&p, "[security]\nprofile = \"root\"\n", None, &[]).is_err(),
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
            None,
            &[]
        )
        .is_err());
        // added key
        assert!(check_write_with(
            &p,
            "[security]\nshell_command_mode = \"warn\"\nprofile = \"root\"\n",
            None,
            &[]
        )
        .is_err());
        // removed table
        assert!(check_write_with(&p, "[project]\nname = \"t\"\n", None, &[]).is_err());
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
                None, &[]),
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
                None,
                &[]
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
        assert!(check_write_with(&p, "[security]\nprofile = \"root\"\n", None, &[]).is_err());
        assert_eq!(
            check_write_with(&p, "[project]\nname = \"t\"\n", None, &[]),
            Ok(())
        );
    }

    /// Content that is not TOML cannot be shown to leave the table alone.
    #[test]
    fn content_that_is_not_toml_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let p = proj(&dir);
        let err = check_write_with(&p, "[security\nprofile = \"root\"\n", None, &[]).unwrap_err();
        assert!(err.contains("not valid TOML"), "{err}");
    }

    /// A broken file on disk is an empty "before": it cannot be used to smuggle a table in.
    #[test]
    fn a_broken_file_on_disk_is_an_empty_before() {
        let dir = tempfile::tempdir().unwrap();
        let p = proj(&dir);
        std::fs::write(&p, "[security\n").unwrap();
        assert!(check_write_with(&p, "[security]\nprofile = \"root\"\n", None, &[]).is_err());
        assert_eq!(
            check_write_with(&p, "[project]\nname = \"t\"\n", None, &[]),
            Ok(())
        );
    }

    /// A file that is THERE but cannot be read as it is (over the cap, not a regular file) is
    /// not an empty "before". Two steps used to drop `[security]` from a protected file: pad it
    /// past the cap with the table unchanged (allowed), then write a small file with no table
    /// (allowed too, because the unreadable "before" compared equal to "no table").
    #[test]
    fn a_file_over_the_cap_refuses_every_write_to_a_protected_path() {
        let dir = tempfile::tempdir().unwrap();
        let p = proj(&dir);
        let cap = crate::util::fs::MAX_CONFIG_FILE_BYTES as usize;
        let secure = "[security]\nshell_command_mode = \"warn\"\n";
        let padded = format!("{secure}#{}\n", "x".repeat(cap));

        std::fs::write(&p, secure).unwrap();
        assert_eq!(
            check_write_with(&p, &padded, None, &[]),
            Ok(()),
            "step 1: the table is unchanged, so padding is allowed while the file is small"
        );
        std::fs::write(&p, &padded).unwrap();

        let small_without_table = "[project]\nname = \"t\"\n";
        let err = check_write_with(&p, small_without_table, None, &[]).unwrap_err();
        assert!(err.contains("cannot be read as it is"), "{err}");
        assert!(err.contains("byte limit"), "names the reason: {err}");
        assert!(
            check_write_with(&p, secure, None, &[]).is_err(),
            "even the same table cannot be shown unchanged against a file that cannot be read"
        );
    }

    /// The same for a file that is not a regular file once links are followed.
    #[cfg(unix)]
    #[test]
    fn a_non_regular_file_at_a_protected_path_refuses_every_write() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = proj(&dir);
        crate::util::fs::special_file_fixtures::make_fifo(&fifo);
        let small = "[project]\nname = \"t\"\n";
        let err = crate::util::fs::special_file_fixtures::within_deadline(
            "check_write_with on a FIFO",
            move || check_write_with(&fifo, small, None, &[]),
        )
        .unwrap_err();
        assert!(err.contains("not a regular file"), "{err}");

        let dir = tempfile::tempdir().unwrap();
        let p = proj(&dir);
        let real = dir.path().join("a-dir");
        std::fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink(&real, &p).unwrap();
        assert!(check_write_with(&p, small, None, &[]).is_err());
    }

    /// The write guard reads the file as it is now. A FIFO there used to hang the write; a file
    /// that is there but cannot be read as it is, is an `Err` (the write is refused), not an
    /// empty "before"; absent, dangling and non-UTF-8 stay empty; a link to a regular file is
    /// still read.
    #[cfg(unix)]
    #[test]
    fn a_fifo_or_a_directory_on_disk_is_an_unreadable_before_and_never_a_hang() {
        use crate::util::fs::special_file_fixtures::{make_fifo, within_deadline};
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("pipe.toml");
        make_fifo(&fifo);
        let got = within_deadline("security_on_disk on a FIFO", move || {
            security_on_disk(&fifo)
        });
        assert!(got.unwrap_err().contains("not a regular file"));

        let real = dir.path().join("a-dir");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("dir-link.toml");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(security_on_disk(&link).is_err());

        let file = dir.path().join("ok.toml");
        std::fs::write(&file, "[security]\nshell_command_mode = \"warn\"\n").unwrap();
        let linked = dir.path().join("ok-link.toml");
        std::os::unix::fs::symlink(&file, &linked).unwrap();
        assert!(
            matches!(security_on_disk(&linked), Ok(Some(_))),
            "a link to a regular file is still read"
        );

        assert_eq!(security_on_disk(&dir.path().join("absent.toml")), Ok(None));
        let dangling = dir.path().join("dangling.toml");
        std::os::unix::fs::symlink(dir.path().join("nowhere"), &dangling).unwrap();
        assert_eq!(
            security_on_disk(&dangling),
            Ok(None),
            "a dangling link is absent"
        );
        let binary = dir.path().join("binary.toml");
        std::fs::write(&binary, [0xffu8, 0xfe]).unwrap();
        assert_eq!(security_on_disk(&binary), Ok(None), "not UTF-8 is not TOML");
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

        assert!(check_write_with(&global, change, Some(&global), &[]).is_err());
        // not the global config when none is named
        assert_eq!(check_write_with(&global, change, None, &[]), Ok(()));
        // another file in the same directory is not it
        assert_eq!(
            check_write_with(&real.join("other.toml"), change, Some(&global), &[]),
            Ok(())
        );
        // the same file NAME in another directory is not it
        let elsewhere = dir.path().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        assert_eq!(
            check_write_with(&elsewhere.join("config.toml"), change, Some(&global), &[]),
            Ok(())
        );
        // a directory that cannot be resolved is not the global config's directory...
        assert_eq!(
            check_write_with(
                &dir.path().join("no-such-dir").join("config.toml"),
                change,
                Some(&global),
                &[]
            ),
            Ok(())
        );
        // ...unless the path IS the one named, which needs no resolving
        let unresolvable = dir.path().join("no-such-global").join("config.toml");
        assert!(check_write_with(&unresolvable, change, Some(&unresolvable), &[]).is_err());
        // the same directory reached through a link
        #[cfg(unix)]
        {
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(&real, &link).unwrap();
            assert!(
                check_write_with(&link.join("config.toml"), change, Some(&global), &[]).is_err()
            );
        }
    }

    /// `resolve` follows a link whose target does not exist, singly and in a chain, relative to
    /// the link's own directory, and stops at a loop instead of spinning.
    #[cfg(unix)]
    #[test]
    fn resolve_follows_dangling_links_and_stops_at_a_loop() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::os::unix::fs::symlink("gone.toml", root.join("one")).unwrap();
        assert_eq!(resolve(&root.join("one")), root.join("gone.toml"), "single");

        std::os::unix::fs::symlink("two-b", root.join("two-a")).unwrap();
        std::os::unix::fs::symlink("end.toml", root.join("two-b")).unwrap();
        assert_eq!(resolve(&root.join("two-a")), root.join("end.toml"), "chain");

        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::os::unix::fs::symlink("../up.toml", root.join("sub").join("rel")).unwrap();
        assert_eq!(resolve(&root.join("sub").join("rel")), root.join("up.toml"));

        std::os::unix::fs::symlink("l2", root.join("l1")).unwrap();
        std::os::unix::fs::symlink("l1", root.join("l2")).unwrap();
        let _ = resolve(&root.join("l1")); // terminates
    }

    /// A hard link to a loaded project's `project.toml` is the same file under another name;
    /// (device, inode) recognises it where comparing paths cannot. A different file that
    /// merely exists is not it. Every existence combination is covered: both exist (ids),
    /// only one exists (never equal), neither exists (resolved paths).
    #[cfg(unix)]
    #[test]
    fn identity_compares_inodes_when_both_exist_and_paths_otherwise() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let cfg = proj(&dir);
        let cfg = std::fs::canonicalize(cfg.parent().unwrap())
            .unwrap()
            .join("project.toml");
        std::fs::write(&cfg, "[project]\nname = \"a\"\n").unwrap();
        let roots = vec![root.clone()];
        let change = "[security]\nprofile = \"root\"\n";

        std::fs::hard_link(&cfg, root.join("alias.txt")).unwrap();
        assert!(
            check_write_with(&root.join("alias.txt"), change, None, &roots).is_err(),
            "hard link"
        );

        std::fs::write(root.join("other.txt"), "x").unwrap();
        assert_eq!(
            check_write_with(&root.join("other.txt"), change, None, &roots),
            Ok(()),
            "both exist, different files"
        );
        assert_eq!(
            check_write_with(&root.join("new.txt"), change, None, &roots),
            Ok(()),
            "target missing, config exists"
        );

        // config missing, target exists
        let empty = tempfile::tempdir().unwrap();
        let eroot = std::fs::canonicalize(empty.path()).unwrap();
        std::fs::write(eroot.join("here.txt"), "x").unwrap();
        assert_eq!(
            check_write_with(
                &eroot.join("here.txt"),
                change,
                None,
                std::slice::from_ref(&eroot)
            ),
            Ok(()),
            "config missing, target exists"
        );
        // neither exists: a create of the config's own path is caught by path
        assert!(check_write_with(
            &eroot.join(".codescout").join("project.toml"),
            change,
            None,
            std::slice::from_ref(&eroot)
        )
        .is_err());
        // neither exists, a different path
        assert_eq!(
            check_write_with(&eroot.join("nope.txt"), change, None, &[eroot]),
            Ok(()),
            "neither exists, different"
        );
    }

    /// `resolve` keeps what follows a link that is a DIRECTORY component: `.codescout` a dangling
    /// link to a missing `cfgdir`, then `cfgdir/project.toml` is the config's own path.
    #[cfg(unix)]
    #[test]
    fn a_dangling_directory_link_keeps_the_rest_of_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::os::unix::fs::symlink("cfgdir", root.join(".codescout")).unwrap();
        let change = "[security]\nprofile = \"root\"\n";
        assert!(check_write_with(
            &root.join("cfgdir").join("project.toml"),
            change,
            None,
            std::slice::from_ref(&root)
        )
        .is_err());
        assert_eq!(
            check_write_with(
                &root.join("cfgdir").join("other.toml"),
                change,
                None,
                &[root]
            ),
            Ok(()),
            "control: another file in that directory is not the config"
        );
    }

    /// When neither file exists the TARGET is resolved too: a second dangling link to the same
    /// missing file names the config as surely as the file itself.
    #[cfg(unix)]
    #[test]
    fn a_second_dangling_link_to_the_same_missing_file_is_the_config() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::os::unix::fs::symlink("../x.toml", root.join(".codescout").join("project.toml"))
            .unwrap();
        std::os::unix::fs::symlink("x.toml", root.join("a.toml")).unwrap();
        let change = "[security]\nprofile = \"root\"\n";
        assert!(check_write_with(&root.join("a.toml"), change, None, &[root]).is_err());
    }
}
