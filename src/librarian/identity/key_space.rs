//! The key-space scope rule: which subtrees of a repo have no artifact key.
//!
//! `.codescout/key-space.toml` at the repo's git root lists them:
//!
//! ```toml
//! exclude = ["contrib", "docs/evals/**"]
//! ```
//!
//! A pattern excludes a path when the glob matches the path or any ancestor directory of
//! it, so `contrib` and `contrib/**` both exclude `contrib/pi/AGENTS.md`. Paths are
//! git-root-relative; a backslash path is judged like its forward-slash form.
//!
//! Read strictly: an unknown key is an error. A missing file is the empty rule (nothing is
//! outside the key space).

use std::path::Path;

use anyhow::Context;
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Deserialize;

/// The key-space scope file, relative to the repo root.
pub const KEY_SPACE_FILE_REL: &str = ".codescout/key-space.toml";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeySpaceFile {
    #[serde(default)]
    exclude: Vec<String>,
}

/// The subtrees that have no artifact key.
#[derive(Debug, Clone)]
pub struct KeySpace {
    /// Each pattern compiled as written and, unless it already ends in `/**`, as `<p>/**`, so
    /// an ancestor directory that matches excludes everything under it without a path walk.
    matchers: GlobSet,
    patterns: Vec<String>,
}

impl KeySpace {
    /// The rule that excludes nothing.
    pub fn empty() -> KeySpace {
        KeySpace {
            matchers: GlobSet::empty(),
            patterns: Vec::new(),
        }
    }

    /// Parse the text of a `key-space.toml`. The one parser behind [`load`] and every loader
    /// that gets the text from elsewhere (a git blob), so they cannot disagree. An error names
    /// [`KEY_SPACE_FILE_REL`].
    pub fn from_text(text: &str) -> anyhow::Result<KeySpace> {
        (|| -> anyhow::Result<KeySpace> {
            let file: KeySpaceFile = toml::from_str(text)?;
            let patterns: Vec<&str> = file.exclude.iter().map(String::as_str).collect();
            KeySpace::from_patterns(&patterns)
        })()
        .with_context(|| format!("parsing {KEY_SPACE_FILE_REL}"))
    }

    /// Compile `patterns`. Each pattern is normalised the way [`KeySpace::is_outside`]
    /// normalises a path (`\` read as `/`; empty and `.` components dropped), so `./contrib`,
    /// `/contrib`, `contrib/` and `contrib` are one pattern. A pattern with no component left
    /// (blank, `./`, `/`) or an invalid glob is an error naming [`KEY_SPACE_FILE_REL`]. A backslash is a
    /// separator, never a glob escape, so a literal `*` or `?` in a name is written as `[*]` or `[?]`.
    pub fn from_patterns(patterns: &[&str]) -> anyhow::Result<KeySpace> {
        let mut builder = GlobSetBuilder::new();
        for &p in patterns {
            let bare = normalise(p.trim());
            anyhow::ensure!(
                !bare.is_empty(),
                "{KEY_SPACE_FILE_REL}: exclude pattern {p:?} names no path"
            );
            let glob = |g: &str| {
                Glob::new(g).with_context(|| format!("{KEY_SPACE_FILE_REL}: exclude pattern {p:?}"))
            };
            builder.add(glob(&bare)?);
            if !bare.ends_with("/**") {
                builder.add(glob(&format!("{bare}/**"))?);
            }
        }
        Ok(KeySpace {
            matchers: builder.build()?,
            patterns: patterns.iter().map(|p| p.to_string()).collect(),
        })
    }

    /// True when `rel_path` (git-root-relative) lies in an excluded subtree. The path is
    /// normalised exactly as `key_of` reads it: `\` is `/`, empty and `.` components are
    /// dropped, `..` stays an ordinary component. A `..` is never resolved, so a caller passes the
    /// repo-relative paths git lists, which have none (the contract `key_of` states).
    pub fn is_outside(&self, rel_path: &str) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        self.matchers.is_match(normalise(rel_path))
    }

    /// The exclude patterns as written, in file order.
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }
}

/// A path or pattern as `key_of` reads a path: the same `components`, joined with `/`.
fn normalise(s: &str) -> String {
    super::key::components(s).join("/")
}

/// Load `.codescout/key-space.toml` under `root`. An absent file is [`KeySpace::empty`]; an
/// unreadable one (a dangling link, a link out of the repository), bad TOML, an unknown key or
/// a bad glob is an error that names the file.
pub fn load(root: &Path) -> anyhow::Result<KeySpace> {
    use crate::librarian::read_door::{for_project, ReadError};

    let path = root.join(KEY_SPACE_FILE_REL);
    // Through the read door, judged against the project: a key-space file that is a link out of
    // the repository is refused like a malformed one.
    let door = for_project(root);
    let text = match door.text(&path) {
        Ok(t) => t,
        Err(ReadError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            // The door reports a dangling leaf link, and a `.codescout` link that leaves the
            // repository, as "not found". Neither is an absent file.
            return match door.refusal(&path) {
                Some(refusal) => {
                    Err(refusal).with_context(|| format!("reading {}", path.display()))
                }
                None => Ok(KeySpace::empty()),
            };
        }
        Err(e) => {
            return Err(e.into_anyhow()).with_context(|| format!("reading {}", path.display()));
        }
    };
    KeySpace::from_text(&text).with_context(|| format!("reading {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(root: &std::path::Path, text: &str) {
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::write(root.join(KEY_SPACE_FILE_REL), text).unwrap();
    }

    #[test]
    fn absent_file_excludes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let ks = load(dir.path()).unwrap();
        assert!(ks.patterns().is_empty());
        assert!(!ks.is_outside("contrib/pi/AGENTS.md"));
        assert!(!ks.is_outside("README.md"));
    }

    #[test]
    fn a_bare_directory_pattern_excludes_its_whole_subtree() {
        let ks = KeySpace::from_patterns(&["contrib"]).unwrap();
        assert!(ks.is_outside("contrib/pi/AGENTS.md"));
        assert!(ks.is_outside("contrib/README.md"));
        assert!(!ks.is_outside("contribution.md"));
        assert!(!ks.is_outside("docs/contrib/x.md"));
    }

    #[test]
    fn a_double_star_pattern_excludes_the_same_subtree() {
        let ks = KeySpace::from_patterns(&["docs/evals/**"]).unwrap();
        assert!(ks.is_outside("docs/evals/a/b.md"));
        assert!(ks.is_outside("docs/evals/a.md"));
        assert!(!ks.is_outside("docs/evaluation.md"));
        assert!(!ks.is_outside("docs/other/a.md"));
        let bare = KeySpace::from_patterns(&["contrib/**"]).unwrap();
        assert!(bare.is_outside("contrib/pi/AGENTS.md"));
    }

    #[test]
    fn a_windows_separator_path_is_judged_like_the_forward_slash_form() {
        let ks = KeySpace::from_patterns(&["contrib", "docs/evals/**"]).unwrap();
        assert!(ks.is_outside("contrib\\pi\\AGENTS.md"));
        assert!(ks.is_outside("docs\\evals\\a.md"));
        assert!(!ks.is_outside("contribution.md"));
        assert!(!ks.is_outside("docs\\other\\a.md"));
    }

    #[test]
    fn a_path_is_normalised_the_way_key_of_reads_it() {
        let ks = KeySpace::from_patterns(&["contrib"]).unwrap();
        for p in [
            "./contrib/x.md",
            "/contrib/x.md",
            "contrib//x.md",
            "./contrib\\x.md",
            ".\\contrib\\x.md",
        ] {
            assert!(ks.is_outside(p), "{p}");
            // The same path gets a key, so the rule must exclude it.
            assert!(crate::librarian::identity::key::key_of(p).is_ok(), "{p}");
        }
        // `..` is an ordinary component, as in key_of.
        assert!(!ks.is_outside("../contrib/x.md"));
        assert!(!ks.is_outside("contribution/x.md"));
    }

    #[test]
    fn a_pattern_is_normalised_like_a_path() {
        for pat in [
            "./contrib",
            "/contrib",
            "contrib/",
            "./contrib/",
            "contrib//",
            ".\\contrib",
        ] {
            let ks = KeySpace::from_patterns(&[pat]).unwrap();
            assert!(ks.is_outside("contrib/pi/AGENTS.md"), "{pat}");
            assert!(!ks.is_outside("contribution.md"), "{pat}");
            assert_eq!(ks.patterns(), [pat], "patterns() keeps the text as written");
        }
        let ks = KeySpace::from_patterns(&["docs//evals/**", "/docs/./x"]).unwrap();
        assert!(ks.is_outside("docs/evals/a.md"));
        assert!(ks.is_outside("docs/x/a.md"));
        let from_file =
            KeySpace::from_text("exclude = [\"./contrib\", \"/docs/evals\"]\n").unwrap();
        assert!(from_file.is_outside("contrib/pi/AGENTS.md"));
        assert!(from_file.is_outside("docs/evals/a.md"));
    }

    #[test]
    fn a_pattern_that_names_no_path_is_an_error_naming_the_file() {
        for pat in ["", "  ", "./", "/", ".", "//", "./."] {
            let err = format!("{:#}", KeySpace::from_patterns(&[pat]).unwrap_err());
            assert!(err.contains(KEY_SPACE_FILE_REL), "{pat:?}: {err}");
            let text = format!("exclude = [{pat:?}]\n");
            let err = format!("{:#}", KeySpace::from_text(&text).unwrap_err());
            assert!(err.contains(KEY_SPACE_FILE_REL), "{pat:?}: {err}");
        }
    }

    #[test]
    fn malformed_toml_is_an_error_naming_the_file() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "exclude = \n");
        let err = format!("{:#}", load(dir.path()).unwrap_err());
        assert!(err.contains(KEY_SPACE_FILE_REL), "{err}");

        write_file(dir.path(), "exclude = [\"a[\"]\n");
        let err = format!("{:#}", load(dir.path()).unwrap_err());
        assert!(err.contains(KEY_SPACE_FILE_REL), "bad glob: {err}");
    }

    #[test]
    fn an_unknown_key_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), "exclude = [\"contrib\"]\nfuture = 1\n");
        let err = format!("{:#}", load(dir.path()).unwrap_err());
        assert!(err.contains(KEY_SPACE_FILE_REL), "{err}");
        assert!(KeySpace::from_text("exclud = []\n").is_err());
    }

    #[test]
    fn from_text_parses_what_load_reads() {
        let text = "exclude = [\"contrib\", \"docs/evals/**\"]\n";
        let dir = tempfile::tempdir().unwrap();
        write_file(dir.path(), text);
        let loaded = load(dir.path()).unwrap();
        let parsed = KeySpace::from_text(text).unwrap();
        assert_eq!(loaded.patterns(), parsed.patterns());
        assert_eq!(parsed.patterns(), ["contrib", "docs/evals/**"]);
        for p in ["contrib/a.md", "docs/evals/x/y.md", "docs/a.md", "b.md"] {
            assert_eq!(loaded.is_outside(p), parsed.is_outside(p), "{p}");
        }
        // An empty file is a valid, empty rule.
        assert!(KeySpace::from_text("").unwrap().patterns().is_empty());
    }

    #[test]
    fn a_star_crosses_a_slash_and_matching_is_anchored_and_case_sensitive() {
        let star = KeySpace::from_patterns(&["a*"]).unwrap();
        assert!(star.is_outside("a/b.md"), "`*` crosses `/`");
        assert!(star.is_outside("abc/d.md"));
        assert!(
            !star.is_outside("x/a/b.md"),
            "a pattern is anchored at the repo root"
        );
        let cased = KeySpace::from_patterns(&["Contrib"]).unwrap();
        assert!(cased.is_outside("Contrib/x.md"));
        assert!(
            !cased.is_outside("contrib/x.md"),
            "matching is case-sensitive"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_codescout_dir_linked_out_of_the_repo_is_an_error_not_absent() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(
            outside.path().join("key-space.toml"),
            "exclude = [\"contrib\"]\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path(), repo.path().join(".codescout")).unwrap();
        let err = load(repo.path()).expect_err("a link that leaves the repo must not be read");
        assert!(format!("{err:#}").contains("key-space.toml"), "{err:#}");
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_link_is_an_error_not_absent() {
        let repo = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("elsewhere.toml");
        std::fs::create_dir_all(repo.path().join(".codescout")).unwrap();
        std::os::unix::fs::symlink(&target, repo.path().join(KEY_SPACE_FILE_REL)).unwrap();
        let err = format!("{:#}", load(repo.path()).unwrap_err());
        assert!(err.contains("key-space.toml"), "{err}");
    }
}
