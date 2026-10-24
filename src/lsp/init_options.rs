//! `initializationOptions` codescout sends with `initialize`, per language server.
//!
//! One caller pair shares this: [`super::client::LspClient`] (a server it spawns itself) and the
//! mux handshake in [`super::mux::process`]. Both must send the same thing, or a workspace would
//! behave differently with and without the mux.
//!
//! Today there is one rule. rust-analyzer scans every directory under the workspace root, and a
//! linked git worktree checked out inside the project (`.claude/worktrees/*`, `.worktrees/*`)
//! multiplies that scan. On this repo, 275,752 files in nested worktrees against 14,909 in the main
//! tree kept `textDocument/documentSymbol` answering `null` for 33 to 54 s (three replicates), so
//! `references` and `symbols` failed after the 5 s cold-start budget
//! (docs/issues/archive/2026-10-23-documentsymbol-null-window-outlasts-the-5s-cold-budget-because-rust-analyzer-scans-the-nested-worktrees.md).
//! With the worktrees excluded the first symbols came after about 7 to 8 s.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

/// True when `server_command` runs rust-analyzer. Matched on the file name, so an absolute path
/// (`~/.cargo/bin/rust-analyzer`) counts and a wrapper with another name does not.
fn is_rust_analyzer(server_command: &str) -> bool {
    Path::new(server_command)
        .file_stem()
        .is_some_and(|stem| stem == "rust-analyzer")
}

/// True for a directory that exists to hold worktrees: its own name is `worktrees` or
/// `.worktrees` (`.claude/worktrees`, `.worktrees`). Such a directory is excluded whole, not
/// child by child, because it also holds trees `git worktree list` does not know and the
/// mutation-probe slots it skips on purpose, and on this repo that is most of its files.
///
/// Only the NAME decides. Any other parent (`crates`, `vendor`) holding one worktree must never
/// be excluded whole: that would hide real source. A path with no name (the workspace root, an
/// empty path) is not one either.
fn is_worktrees_dir(relative: &Path) -> bool {
    relative
        .file_name()
        .is_some_and(|name| name == "worktrees" || name == ".worktrees")
}

/// Whether codescout advertises `window.workDoneProgress` to `server_command`, so that the server
/// sends `$/progress` for its start-up work (which [`super::progress`] reads to decide how long a
/// `null` answer is re-asked). rust-analyzer only: another server (kotlin-lsp above all) keeps the
/// capabilities it has always seen, because an advertised capability makes it send a stream it
/// never sent before.
pub(crate) fn wants_progress(server_command: &str) -> bool {
    is_rust_analyzer(server_command)
}

/// The `initializationOptions` for `server_command` in `workspace_root`, given the linked
/// worktrees of the repository (`worktrees`, absolute roots). `None` means "send none".
///
/// Pure: the caller supplies the worktrees, so a test needs no git repository.
pub(crate) fn initialization_options(
    server_command: &str,
    workspace_root: &Path,
    worktrees: &[PathBuf],
) -> Option<Value> {
    if !is_rust_analyzer(server_command) {
        return None;
    }
    let mut exclude: Vec<String> = worktrees
        .iter()
        .filter_map(|worktree| worktree.strip_prefix(workspace_root).ok())
        // The root itself strips to the empty path; excluding that would exclude everything.
        .filter(|relative| relative.components().next().is_some())
        .map(|relative| {
            // A directory dedicated to worktrees is excluded whole (see `is_worktrees_dir`).
            let excluded = match relative.parent() {
                Some(parent) if is_worktrees_dir(parent) => parent,
                _ => relative,
            };
            excluded
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect();
    if exclude.is_empty() {
        return None;
    }
    exclude.sort();
    exclude.dedup();
    Some(json!({"files": {"excludeDirs": exclude}}))
}

/// [`initialization_options`] with the worktrees read from the repository at `workspace_root`.
pub(crate) fn for_workspace(server_command: &str, workspace_root: &Path) -> Option<Value> {
    initialization_options(
        server_command,
        workspace_root,
        &crate::util::path_security::list_git_worktrees(workspace_root),
    )
}

/// Setup shared by the two wire tests (`LspClient::start` and the mux handshake): a repository
/// with one linked worktree nested in it, a command whose FILE NAME is chosen, and a fake server
/// that records the `initialize` params it receives.
#[cfg(all(test, unix))]
pub(crate) mod test_support {
    use std::path::PathBuf;

    pub(crate) struct FakeServer {
        /// Owns the files below; dropped last.
        pub(crate) _dir: tempfile::TempDir,
        pub(crate) workspace: PathBuf,
        /// A symlink named `command_name` to the real python3 interpreter.
        pub(crate) command: PathBuf,
        /// Arguments for `command`: the fixture script, then the file it records into.
        pub(crate) args: Vec<String>,
        /// Where the fixture writes the `initialize` params.
        pub(crate) recorded: PathBuf,
    }

    impl FakeServer {
        /// The params the fixture recorded, once it has received `initialize`.
        pub(crate) fn recorded_params(&self) -> Option<serde_json::Value> {
            serde_json::from_str(&std::fs::read_to_string(&self.recorded).ok()?).ok()
        }
    }

    pub(crate) fn fake_server(command_name: &str) -> FakeServer {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("ws");
        // What `git worktree add` leaves: `.git/worktrees/<name>/gitdir` naming the worktree's `.git`.
        let registration = workspace.join(".git").join("worktrees").join("wt");
        std::fs::create_dir_all(&registration).unwrap();
        std::fs::write(
            registration.join("gitdir"),
            format!("{}\n", workspace.join(".worktrees/wt/.git").display()),
        )
        .unwrap();

        // The command's FILE NAME selects the rust-analyzer rule, so a symlink with that name
        // stands in for it (a link is not an exec of a file the test has just written).
        // `python3` on PATH may be a shim (pyenv) that dispatches on its own name, so link the
        // real interpreter, not whatever the name resolves to.
        let python = std::process::Command::new("python3")
            .args(["-c", "import sys; print(sys.executable)"])
            .output()
            .expect("python3 is required (the gate runs the Python tests too)")
            .stdout;
        let python = String::from_utf8(python).unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let command = bin.join(command_name);
        std::os::unix::fs::symlink(python.trim(), &command).unwrap();

        let recorded = dir.path().join("initialize-params.json");
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/fake_lsp_records_initialize.py");
        FakeServer {
            args: vec![
                fixture.to_string_lossy().into_owned(),
                recorded.to_string_lossy().into_owned(),
            ],
            _dir: dir,
            workspace,
            command,
            recorded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/repo")
    }

    #[test]
    fn rust_analyzer_gets_the_nested_worktrees_excluded() {
        let worktrees = vec![
            PathBuf::from("/repo/crates/b-wt"),
            PathBuf::from("/repo/a-wt"),
        ];
        let options = initialization_options("rust-analyzer", &root(), &worktrees)
            .expect("nested worktrees must produce options");
        assert_eq!(
            options,
            json!({"files": {"excludeDirs": ["a-wt", "crates/b-wt"]}}),
            "relative to the workspace root, in a stable (sorted) order; the ordinary parent \
             `crates` is never excluded, only the worktree itself"
        );
    }

    /// On this repo the registered worktrees were not enough: the null window stayed 31 s with
    /// all 43 of them excluded, and fell to 7 to 8 s with `.claude/worktrees` and `.worktrees`
    /// excluded whole. Those directories also hold trees that are not registered (and the
    /// mutation-probe slots, which the worktree list skips on purpose), and they are the bulk of
    /// the files. A directory DEDICATED to worktrees is therefore excluded whole.
    #[test]
    fn a_directory_dedicated_to_worktrees_is_excluded_whole() {
        let worktrees = vec![
            PathBuf::from("/repo/.claude/worktrees/agent-1"),
            PathBuf::from("/repo/.claude/worktrees/agent-2"),
            PathBuf::from("/repo/.worktrees/albert"),
        ];
        assert_eq!(
            initialization_options("rust-analyzer", &root(), &worktrees),
            Some(json!({"files": {"excludeDirs": [".claude/worktrees", ".worktrees"]}})),
            "the whole directory, not each child (a child is covered by its parent)"
        );
    }

    /// A parent is excluded only when its NAME says it is for worktrees. `crates` or `vendor`
    /// holding one worktree must never be excluded whole: that would hide real source.
    #[test]
    fn an_ordinary_parent_directory_is_never_excluded() {
        let worktrees = vec![PathBuf::from("/repo/crates/scratch-wt")];
        assert_eq!(
            initialization_options("rust-analyzer", &root(), &worktrees),
            Some(json!({"files": {"excludeDirs": ["crates/scratch-wt"]}}))
        );
    }

    #[test]
    fn a_worktree_outside_the_root_is_not_excluded() {
        let worktrees = vec![PathBuf::from("/elsewhere/wt")];
        assert_eq!(
            initialization_options("rust-analyzer", &root(), &worktrees),
            None,
            "rust-analyzer never scans a directory outside the workspace root"
        );
    }

    #[test]
    fn the_root_itself_is_never_excluded() {
        let worktrees = vec![root()];
        assert_eq!(
            initialization_options("rust-analyzer", &root(), &worktrees),
            None,
            "excluding the empty relative path would exclude the whole workspace"
        );
    }

    #[test]
    fn a_server_that_is_not_rust_analyzer_gets_nothing() {
        let worktrees = vec![PathBuf::from("/repo/.worktrees/a")];
        for command in [
            "kotlin-lsp",
            "pyright-langserver",
            "typescript-language-server",
        ] {
            assert_eq!(
                initialization_options(command, &root(), &worktrees),
                None,
                "{command}: `files.excludeDirs` is a rust-analyzer setting"
            );
        }
    }

    #[test]
    fn only_rust_analyzer_is_sent_the_progress_capability() {
        assert!(wants_progress("rust-analyzer"));
        assert!(wants_progress("/home/u/.cargo/bin/rust-analyzer"));
        for command in [
            "kotlin-lsp",
            "pyright-langserver",
            "my-rust-analyzer-wrapper",
        ] {
            assert!(!wants_progress(command), "{command}");
        }
    }

    #[test]
    fn no_worktrees_means_no_options() {
        assert_eq!(initialization_options("rust-analyzer", &root(), &[]), None);
    }

    #[test]
    fn the_command_is_matched_by_file_name() {
        let worktrees = vec![PathBuf::from("/repo/.worktrees/a")];
        assert!(
            initialization_options("/home/u/.cargo/bin/rust-analyzer", &root(), &worktrees)
                .is_some(),
            "an absolute path to rust-analyzer counts"
        );
        assert_eq!(
            initialization_options("my-rust-analyzer-wrapper", &root(), &worktrees),
            None,
            "a wrapper under another name is not assumed to be rust-analyzer"
        );
    }
}
