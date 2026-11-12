//! The repo name: the human-readable first segment of a document key.
//!
//! A repo names itself in `.codescout/repo.toml` (`name = "<name>"`) at the repo's git root, or
//! at the project root when there is no git. Keys are repo-relative paths, so every project in
//! one repo shares one key space and one name. The name
//! is lowercase `[a-z0-9]` runs joined by single `-`, and is never exactly 16
//! hex characters, so it cannot be mistaken for a 16-hex repo id.

use std::fmt;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

/// The repo-name file, relative to the repo root.
pub const REPO_FILE_REL: &str = ".codescout/repo.toml";

/// The workspace file whose `[workspace] name` seeds [`suggest`].
const WORKSPACE_FILE_REL: &str = ".codescout/workspace.toml";

/// Fallback suggestion when neither the workspace name nor the root basename
/// yields a usable name.
const FALLBACK_NAME: &str = "repo";

/// `^[a-z0-9]+(-[a-z0-9]+)*$`.
pub fn is_canonical_name(s: &str) -> bool {
    !s.is_empty()
        && s.split('-').all(|seg| {
            !seg.is_empty()
                && seg
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// Exactly 16 characters of `[0-9a-f]`, the shape of a repo id.
pub fn is_hex16(s: &str) -> bool {
    s.len() == 16
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A validated repo name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoName(String);

/// Why a string is not a repo name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    /// Outside the charset `^[a-z0-9]+(-[a-z0-9]+)*$`.
    NonCanonical(String),
    /// Exactly 16 hex characters: indistinguishable from a repo id.
    HexShaped(String),
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameError::NonCanonical(s) => write!(
                f,
                "repo name {s:?} must match ^[a-z0-9]+(-[a-z0-9]+)*$ \
                 (lowercase letters and digits, single '-' between runs)"
            ),
            NameError::HexShaped(s) => write!(
                f,
                "repo name {s:?} is exactly 16 hex characters, which is the shape of a repo id; \
                 choose another name"
            ),
        }
    }
}

impl std::error::Error for NameError {}

impl RepoName {
    pub fn parse(s: &str) -> Result<RepoName, NameError> {
        if !is_canonical_name(s) {
            return Err(NameError::NonCanonical(s.to_string()));
        }
        if is_hex16(s) {
            return Err(NameError::HexShaped(s.to_string()));
        }
        Ok(RepoName(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Fold `s` toward a canonical name: ASCII lowercase, every run of characters
/// outside `[a-z0-9]` (any non-ASCII character included) becomes one `-`,
/// leading and trailing `-` trimmed.
/// `None` when nothing is left or the result is hex16-shaped.
pub fn canonicalize(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut pending_dash = false;
    for c in s.chars().map(|c| c.to_ascii_lowercase()) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c);
        } else {
            pending_dash = true;
        }
    }
    if out.is_empty() || is_hex16(&out) {
        None
    } else {
        Some(out)
    }
}

/// `.codescout/repo.toml`. Unknown fields are tolerated on purpose: an older
/// binary must survive a future field.
#[derive(Deserialize)]
struct RepoFile {
    name: String,
}

/// The repo's declared name, `Ok(None)` when `.codescout/repo.toml` is absent.
/// A present but unreadable, malformed or non-canonical file is an error that
/// names the file.
pub fn read(root: &Path) -> anyhow::Result<Option<RepoName>> {
    use crate::librarian::read_door::{for_project, ReadError};

    let path = root.join(REPO_FILE_REL);
    // Through the read door, judged against the project: a repo.toml that is a link out of the
    // repository is refused like a malformed one.
    let door = for_project(root);
    let text = match door.text(&path) {
        Ok(t) => t,
        Err(ReadError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            // The door reports a dangling leaf link, and a `.codescout` link that leaves the
            // repository, as "not found". Neither is an absent file: `write` must not follow
            // them, so they are errors here and the caller never sees `None` for them.
            return match door.refusal(&path) {
                Some(refusal) => {
                    Err(refusal).with_context(|| format!("reading {}", path.display()))
                }
                None => Ok(None),
            };
        }
        Err(e) => {
            return Err(e.into_anyhow()).with_context(|| format!("reading {}", path.display()));
        }
    };
    let parsed = parse_text(&text).with_context(|| format!("reading {}", path.display()))?;
    Ok(Some(parsed))
}

/// Parse the text of a `.codescout/repo.toml`: the one parser behind [`read`] and every loader
/// that gets the text from elsewhere (a git blob), so they cannot disagree. The caller adds the
/// file's name to the error.
pub fn parse_text(text: &str) -> anyhow::Result<RepoName> {
    let file: RepoFile = toml::from_str(text)?;
    Ok(RepoName::parse(&file.name)?)
}

/// Write `.codescout/repo.toml` under `root`, creating `.codescout/` if needed.
/// Returns the file path.
pub fn write(root: &Path, name: &RepoName) -> anyhow::Result<PathBuf> {
    use crate::librarian::util::{refuse_catalogued_path_outside_repo, write_catalogued_file};

    let path = root.join(REPO_FILE_REL);
    let roots = [root.to_path_buf()];
    // Judge the target before `create_dir_all` builds anything: a leaf link is never written
    // through, and a `.codescout` link that leaves the repository is refused.
    crate::librarian::util::refuse_write_through_symlink(&path)?;
    refuse_catalogued_path_outside_repo(&roots, &path, "write")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    write_catalogued_file(&roots, &path, format!("name = \"{name}\"\n"))
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// A name to offer when the repo has none: the `[workspace] name` of
/// `.codescout/workspace.toml`, else the root's basename, else `repo`. Each
/// goes through [`canonicalize`].
///
/// A linked worktree offers what its main checkout would: `workspace.toml` is gitignored, so a
/// worktree never has one, and a worktree's folder name is not the repo's name. Both lookups use
/// the main checkout's root there. Only a linked worktree is redirected (the gate every other
/// librarian caller uses): a submodule's `.git` file has no `worktrees` component, so a submodule
/// keeps its own root, name and `workspace.toml`. A relative `gitdir:` pointer is relative to
/// `root`, never to the process's working directory.
pub fn suggest(root: &Path) -> RepoName {
    use crate::util::path_security::{is_linked_worktree, worktree_main_root};

    let root = if is_linked_worktree(root) {
        worktree_main_root(root)
            .map(|main| {
                let main = if main.is_relative() {
                    root.join(main)
                } else {
                    main
                };
                std::fs::canonicalize(&main).unwrap_or(main)
            })
            .unwrap_or_else(|| root.to_path_buf())
    } else {
        root.to_path_buf()
    };
    let root = root.as_path();
    let workspace_file = root.join(WORKSPACE_FILE_REL);
    let from_workspace = crate::librarian::read_door::for_project(root)
        .text(&workspace_file)
        .ok()
        .and_then(|t| toml::from_str::<toml::Value>(&t).ok())
        .and_then(|v| {
            v.get("workspace")?
                .get("name")?
                .as_str()
                .map(str::to_string)
        })
        .and_then(|n| canonicalize(&n));
    let from_basename = || {
        root.file_name()
            .and_then(|n| n.to_str())
            .and_then(canonicalize)
    };
    from_workspace
        .or_else(from_basename)
        .and_then(|n| RepoName::parse(&n).ok())
        .unwrap_or_else(|| RepoName(FALLBACK_NAME.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_names_are_accepted_and_others_refused() {
        assert!(RepoName::parse("codescout").is_ok());
        assert!(RepoName::parse("claude-plugins").is_ok());
        assert!(RepoName::parse("mrv-poc2").is_ok());
        for bad in [
            "",
            "Codescout",
            "claude_plugins",
            "-x",
            "x-",
            "a--b",
            "two words",
            "ü",
        ] {
            assert!(
                matches!(RepoName::parse(bad), Err(NameError::NonCanonical(_))),
                "{bad}"
            );
        }
        assert!(matches!(
            RepoName::parse("deadbeefcafe1234"),
            Err(NameError::HexShaped(_))
        ));
        assert!(RepoName::parse("deadbeefcafe12345").is_ok()); // 17 chars is not id-shaped
    }

    #[test]
    fn canonicalize_folds_case_and_separators() {
        assert_eq!(canonicalize("My_Repo.v2").as_deref(), Some("my-repo-v2"));
        assert_eq!(canonicalize("MRV-poc").as_deref(), Some("mrv-poc"));
        assert_eq!(canonicalize("__"), None);
        assert_eq!(canonicalize("DEADBEEFCAFE1234"), None);
    }

    #[test]
    fn read_is_none_when_absent_and_round_trips_after_write() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(dir.path()).unwrap(), None);
        let p = write(dir.path(), &RepoName::parse("codescout").unwrap()).unwrap();
        assert_eq!(p, dir.path().join(REPO_FILE_REL));
        assert_eq!(read(dir.path()).unwrap().unwrap().as_str(), "codescout");
    }

    #[test]
    fn an_unknown_field_does_not_break_read() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(REPO_FILE_REL),
            "name = \"codescout\"\nfuture = 1\n",
        )
        .unwrap();
        assert_eq!(read(dir.path()).unwrap().unwrap().as_str(), "codescout");
    }

    #[test]
    fn a_malformed_file_is_an_error_naming_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(dir.path().join(REPO_FILE_REL), "name = \"Bad Name\"\n").unwrap();
        let err = format!("{:#}", read(dir.path()).unwrap_err());
        assert!(err.contains("repo.toml"), "{err}");
        std::fs::write(dir.path().join(REPO_FILE_REL), "name = \n").unwrap();
        assert!(format!("{:#}", read(dir.path()).unwrap_err()).contains("repo.toml"));
    }

    #[test]
    fn parse_text_is_the_parser_read_uses() {
        assert_eq!(
            parse_text("name = \"codescout\"\nfuture = 1\n")
                .unwrap()
                .as_str(),
            "codescout"
        );
        // The same refusals `read` reports, minus the file name the caller adds.
        for bad in [
            "name = \"Bad Name\"\n",
            "name = \n",
            "",
            "name = \"deadbeefcafe1234\"\n",
        ] {
            assert!(parse_text(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn suggest_prefers_the_workspace_name_then_the_basename() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("My_Project");
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        assert_eq!(suggest(&root).as_str(), "my-project");
        std::fs::write(
            root.join(".codescout/workspace.toml"),
            "[workspace]\nname = \"Code Scout\"\n",
        )
        .unwrap();
        assert_eq!(suggest(&root).as_str(), "code-scout");
    }
    /// Builds `<tmp>/MyRepo` (a `.git` directory) and `<tmp>/wt` (a linked worktree whose `.git`
    /// file points into `MyRepo/.git/worktrees/wt`). Returns `(tmp, main, worktree)`.
    fn main_and_linked_worktree() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("MyRepo");
        let wt = tmp.path().join("wt");
        let admin = main.join(".git/worktrees/wt");
        std::fs::create_dir_all(&admin).unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(wt.join(".git"), format!("gitdir: {}\n", admin.display())).unwrap();
        (tmp, main, wt)
    }

    // `.codescout/workspace.toml` is gitignored, so a linked worktree never has one; the name the
    // main checkout would offer is the one to offer there too (a worktree's folder name is not a
    // repo name, and committing it would collide with the main checkout's `repo.toml`).
    #[test]
    fn suggest_in_a_linked_worktree_uses_the_main_checkouts_workspace_name() {
        let (_tmp, main, wt) = main_and_linked_worktree();
        std::fs::create_dir_all(main.join(".codescout")).unwrap();
        std::fs::write(
            main.join(".codescout/workspace.toml"),
            "[workspace]\nname = \"Code Scout\"\n",
        )
        .unwrap();
        assert_eq!(suggest(&wt).as_str(), "code-scout");
        assert_eq!(suggest(&main).as_str(), "code-scout");
    }

    #[test]
    fn suggest_in_a_linked_worktree_falls_back_to_the_main_checkouts_basename() {
        let (_tmp, main, wt) = main_and_linked_worktree();
        assert_eq!(suggest(&wt).as_str(), "myrepo");
        assert_eq!(suggest(&main).as_str(), "myrepo");
    }

    // The repo's name belongs to the repo, so the main checkout's workspace name is used even when
    // the worktree has a workspace.toml of its own.
    #[test]
    fn a_linked_worktrees_own_workspace_name_is_not_the_repos_name() {
        let (_tmp, main, wt) = main_and_linked_worktree();
        for (root, name) in [(&main, "Main Name"), (&wt, "Own Name")] {
            std::fs::create_dir_all(root.join(".codescout")).unwrap();
            std::fs::write(
                root.join(".codescout/workspace.toml"),
                format!("[workspace]\nname = \"{name}\"\n"),
            )
            .unwrap();
        }
        assert_eq!(suggest(&wt).as_str(), "main-name");
    }

    // A directory that is not a linked worktree behaves as before: its own basename.
    #[test]
    fn suggest_in_a_plain_or_main_checkout_directory_is_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let plain = tmp.path().join("Plain_Dir");
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(suggest(&plain).as_str(), "plain-dir");
        // `.git` as a directory is a main checkout, not a pointer.
        let main = tmp.path().join("Main_Dir");
        std::fs::create_dir_all(main.join(".git")).unwrap();
        assert_eq!(suggest(&main).as_str(), "main-dir");
    }

    /// `<tmp>/super` (a `.git` directory) holding the submodule `<tmp>/super/lib`, whose `.git`
    /// file carries `gitdir`. The submodule's own `.git` file has no `worktrees` component.
    fn submodule(gitdir: impl Fn(&Path) -> String) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let sup = tmp.path().join("super");
        let lib = sup.join("lib");
        std::fs::create_dir_all(sup.join(".git/modules/lib")).unwrap();
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join(".git"), format!("gitdir: {}\n", gitdir(&sup))).unwrap();
        (tmp, lib)
    }

    // A submodule is not a linked worktree: its `.git` file points into `.git/modules/`, and the
    // superproject's name is not the submodule's.
    #[test]
    fn suggest_in_a_submodule_is_the_submodules_own_basename() {
        let (_tmp, lib) = submodule(|_| "../.git/modules/lib".to_string());
        assert_eq!(suggest(&lib).as_str(), "lib");
        let (_tmp, lib) = submodule(|sup| sup.join(".git/modules/lib").display().to_string());
        assert_eq!(suggest(&lib).as_str(), "lib");
    }

    #[test]
    fn suggest_in_a_submodule_uses_its_own_workspace_name() {
        for relative in [true, false] {
            let (_tmp, lib) = submodule(|sup| {
                if relative {
                    "../.git/modules/lib".to_string()
                } else {
                    sup.join(".git/modules/lib").display().to_string()
                }
            });
            let sup = lib.parent().unwrap();
            for (root, name) in [(sup, "Super Name"), (lib.as_path(), "Own Lib")] {
                std::fs::create_dir_all(root.join(".codescout")).unwrap();
                std::fs::write(
                    root.join(".codescout/workspace.toml"),
                    format!("[workspace]\nname = \"{name}\"\n"),
                )
                .unwrap();
            }
            assert_eq!(suggest(&lib).as_str(), "own-lib", "relative={relative}");
        }
    }

    // A relative `gitdir:` pointer is relative to the worktree, not to the process's working
    // directory: `../MyRepo/.git/worktrees/wt` from `<tmp>/wt` is `<tmp>/MyRepo`.
    #[test]
    fn a_relative_worktree_pointer_resolves_against_the_worktree_root() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("MyRepo");
        let wt = tmp.path().join("wt");
        std::fs::create_dir_all(main.join(".git/worktrees/wt")).unwrap();
        std::fs::create_dir_all(main.join(".codescout")).unwrap();
        std::fs::write(
            main.join(".codescout/workspace.toml"),
            "[workspace]\nname = \"Code Scout\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(wt.join(".git"), "gitdir: ../MyRepo/.git/worktrees/wt\n").unwrap();
        // The test process runs elsewhere, so a read through its working directory cannot see
        // `../MyRepo/.codescout/workspace.toml`.
        assert_ne!(std::env::current_dir().unwrap(), tmp.path());
        assert_eq!(suggest(&wt).as_str(), "code-scout");
    }

    // ASCII-only, like `key_of`: U+212A KELVIN SIGN lowercases to `k` under Unicode rules, but it
    // is not in `[a-z0-9]` and so is a separator here.
    #[test]
    fn canonicalize_is_ascii_only() {
        assert_eq!(canonicalize("\u{212A}ey").as_deref(), Some("ey"));
        assert_eq!(canonicalize("a\u{212A}b").as_deref(), Some("a-b"));
        assert_eq!(canonicalize("\u{212A}"), None);
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_repo_toml_link_is_refused_not_absent_and_never_written_through() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("elsewhere.toml");
        std::fs::create_dir_all(repo.path().join(".codescout")).unwrap();
        std::os::unix::fs::symlink(&target, repo.path().join(REPO_FILE_REL)).unwrap();

        let err = format!("{:#}", read(repo.path()).unwrap_err());
        assert!(err.contains("repo.toml"), "{err}");

        let err = format!(
            "{:#}",
            write(repo.path(), &RepoName::parse("codescout").unwrap()).unwrap_err()
        );
        assert!(err.contains("repo.toml"), "{err}");
        assert!(
            !target.exists(),
            "write followed the link and created its target"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_codescout_dir_linked_out_of_the_repo_is_refused_for_read_and_write() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), repo.path().join(".codescout")).unwrap();

        let err = format!("{:#}", read(repo.path()).unwrap_err());
        assert!(err.contains("repo.toml"), "{err}");

        assert!(write(repo.path(), &RepoName::parse("codescout").unwrap()).is_err());
        assert!(
            !outside.path().join("repo.toml").exists(),
            "write landed outside the repository"
        );
    }
}
