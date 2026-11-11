//! The document key: the lowercase filename stem of a markdown document.
//!
//! A `readme.md` takes the name of its folder (`docs/research/README.md` has
//! the key `research`); a root-level readme has the key `readme`. The key must
//! match `^[a-z0-9]+(-[a-z0-9]+)*$` and must not be exactly 16 hex characters
//! (the shape of a repo id). A path that cannot give such a key is an error:
//! this module never guesses or folds a key.
//!
//! Pure string logic: no file reads, no I/O.

use std::fmt;

use super::repo_name::{is_canonical_name, is_hex16};

/// A validated document key.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(String);

/// Why a path or string has no document key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    /// The file name does not end in `.md` (case-insensitive).
    NotMarkdown(String),
    /// The derived key is outside `^[a-z0-9]+(-[a-z0-9]+)*$`.
    NonCanonical { rel_path: String, key: String },
    /// The derived key is exactly 16 hex characters: indistinguishable from a repo id.
    HexShaped { rel_path: String, key: String },
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::NotMarkdown(p) => {
                write!(f, "{p:?} is not a markdown file (its name must end in .md)")
            }
            KeyError::NonCanonical { rel_path, key } => write!(
                f,
                "{rel_path:?} gives the key {key:?}, which must match ^[a-z0-9]+(-[a-z0-9]+)*$ \
                 (lowercase letters and digits, single '-' between runs); rename the file"
            ),
            KeyError::HexShaped { rel_path, key } => write!(
                f,
                "{rel_path:?} gives the key {key:?}, which is exactly 16 hex characters, the \
                 shape of a repo id; rename the file"
            ),
        }
    }
}

impl std::error::Error for KeyError {}

impl Key {
    /// Validate `s` as a key as written: the charset and the hex16 refusal,
    /// with no lowercasing. The error carries `s` as both path and key.
    pub fn parse(s: &str) -> Result<Key, KeyError> {
        Key::checked(s, s.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The single place the key rules are applied; `rel_path` is only for the error.
    fn checked(rel_path: &str, key: String) -> Result<Key, KeyError> {
        if !is_canonical_name(&key) {
            return Err(KeyError::NonCanonical {
                rel_path: rel_path.to_string(),
                key,
            });
        }
        if is_hex16(&key) {
            return Err(KeyError::HexShaped {
                rel_path: rel_path.to_string(),
                key,
            });
        }
        Ok(Key(key))
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The path's components with `\` read as `/`; empty and `.` components dropped.
fn components(rel_path: &str) -> Vec<String> {
    rel_path
        .replace('\\', "/")
        .split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .map(str::to_string)
        .collect()
}

/// The stem of a file name when its extension is `md` (any case); `None` otherwise.
fn markdown_stem(file_name: &str) -> Option<&str> {
    let (stem, ext) = file_name.rsplit_once('.')?;
    ext.eq_ignore_ascii_case("md").then_some(stem)
}

/// True when the file name is `readme.md`, case-insensitive.
pub fn is_folder_index(rel_path: &str) -> bool {
    components(rel_path)
        .last()
        .is_some_and(|name| name.eq_ignore_ascii_case("readme.md"))
}

/// The key of the markdown document at `rel_path`.
pub fn key_of(rel_path: &str) -> Result<Key, KeyError> {
    let parts = components(rel_path);
    let Some(stem) = parts
        .last()
        .and_then(|name| markdown_stem(name))
        .map(str::to_lowercase)
    else {
        return Err(KeyError::NotMarkdown(rel_path.to_string()));
    };
    let key = if is_folder_index(rel_path) {
        match parts.len().checked_sub(2).and_then(|i| parts.get(i)) {
            Some(folder) => folder.to_lowercase(),
            None => stem,
        }
    } else {
        stem
    };
    Key::checked(rel_path, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_is_the_lowercase_stem() {
        assert_eq!(
            key_of("docs/trackers/tracker-hygiene-log.md")
                .unwrap()
                .as_str(),
            "tracker-hygiene-log"
        );
        assert_eq!(key_of("CLAUDE.md").unwrap().as_str(), "claude");
        assert_eq!(
            key_of("docs/issues/2026-11-11-reindex-deletes.md")
                .unwrap()
                .as_str(),
            "2026-11-11-reindex-deletes"
        );
    }

    #[test]
    fn a_readme_takes_its_folder_name() {
        assert_eq!(
            key_of("docs/trackers/ledgers/frictions/readme.md")
                .unwrap()
                .as_str(),
            "frictions"
        );
        assert_eq!(
            key_of("docs/research/README.md").unwrap().as_str(),
            "research"
        );
        assert_eq!(key_of("README.md").unwrap().as_str(), "readme");
        assert_eq!(
            key_of(r"docs\trackers\frictions\README.md")
                .unwrap()
                .as_str(),
            "frictions"
        );
        assert!(is_folder_index("x/ReadMe.md"));
        assert!(!is_folder_index("x/readme-notes.md"));
    }

    #[test]
    fn non_canonical_and_hex_shaped_keys_are_errors_not_guesses() {
        assert!(matches!(
            key_of("docs/Ünïcode Notes.md"),
            Err(KeyError::NonCanonical { .. })
        ));
        assert!(matches!(
            key_of("docs/v1.2-notes.md"),
            Err(KeyError::NonCanonical { .. })
        ));
        assert!(matches!(
            key_of("docs/issue_clusters.md"),
            Err(KeyError::NonCanonical { .. })
        ));
        assert!(matches!(
            key_of("docs/Deadbeefcafe1234.md"),
            Err(KeyError::HexShaped { .. })
        ));
        assert!(matches!(
            key_of("docs/a.txt"),
            Err(KeyError::NotMarkdown(_))
        ));
        assert!(matches!(
            key_of("docs/Bad_Folder/readme.md"),
            Err(KeyError::NonCanonical { .. })
        ));
    }

    #[test]
    fn a_path_with_no_stem_is_non_canonical_not_a_key() {
        // `.md` is a markdown name with an empty stem: the empty key is refused.
        for p in [".md", "docs/.md"] {
            assert!(
                matches!(key_of(p), Err(KeyError::NonCanonical { ref key, .. }) if key.is_empty()),
                "{p}"
            );
        }
    }

    #[test]
    fn a_stem_of_only_dots_is_non_canonical() {
        // `...md` is stem `..` plus extension `md`; a dot is never in the charset.
        assert!(matches!(
            key_of("docs/...md"),
            Err(KeyError::NonCanonical { ref key, .. }) if key == ".."
        ));
    }

    #[test]
    fn leading_dot_slash_leading_slash_and_double_slash_do_not_change_the_key() {
        // Empty and `.` components carry no meaning, so they are dropped before the key is read.
        for p in [
            "./docs/notes.md",
            "/docs/notes.md",
            "docs//notes.md",
            "docs/./notes.md",
        ] {
            assert_eq!(key_of(p).unwrap().as_str(), "notes", "{p}");
        }
        // The folder of a readme is read after the same dropping.
        assert_eq!(key_of("./docs//README.md").unwrap().as_str(), "docs");
        assert_eq!(key_of("./README.md").unwrap().as_str(), "readme");
        assert_eq!(key_of("/README.md").unwrap().as_str(), "readme");
    }

    #[test]
    fn an_uppercase_extension_is_markdown() {
        // The extension is matched case-insensitively, like `README.md`.
        assert_eq!(key_of("docs/NOTES.MD").unwrap().as_str(), "notes");
        assert!(is_folder_index("docs/README.MD"));
    }

    #[test]
    fn no_file_name_is_not_markdown() {
        // An empty path, a bare directory path and a file named `md` have no `.md` extension.
        for p in ["", "docs/", "md"] {
            assert!(matches!(key_of(p), Err(KeyError::NotMarkdown(_))), "{p:?}");
        }
    }

    #[test]
    fn a_readme_in_a_bad_folder_names_the_path_and_the_derived_key() {
        // The error carries the rel path as given and the lowercased folder name it derived.
        assert_eq!(
            key_of("docs/Bad_Folder/README.md"),
            Err(KeyError::NonCanonical {
                rel_path: "docs/Bad_Folder/README.md".into(),
                key: "bad_folder".into(),
            })
        );
        // A hex16-shaped folder name is refused for its readme the same way.
        assert_eq!(
            key_of("docs/DEADBEEFCAFE1234/readme.md"),
            Err(KeyError::HexShaped {
                rel_path: "docs/DEADBEEFCAFE1234/readme.md".into(),
                key: "deadbeefcafe1234".into(),
            })
        );
    }

    #[test]
    fn parse_checks_charset_and_hex16_without_lowercasing() {
        // `parse` takes a key as written: uppercase is a charset error, not folded.
        assert_eq!(Key::parse("frictions").unwrap().as_str(), "frictions");
        assert!(matches!(
            Key::parse("Frictions"),
            Err(KeyError::NonCanonical { .. })
        ));
        assert!(matches!(
            Key::parse("deadbeefcafe1234"),
            Err(KeyError::HexShaped { .. })
        ));
        assert!(matches!(Key::parse(""), Err(KeyError::NonCanonical { .. })));
    }
}
