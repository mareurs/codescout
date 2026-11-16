//! Which markdown files a tool other than the markdown scanner reads by path (D-R26).
//!
//! The cutover never edits or renames such a file: code that reads `docs/x.md` by path would
//! break, and a prompt surface edited by the tool would change what a model is told. A file is
//! frozen when code of `HEAD` names it, when it lies under a dot directory, or when it is a page
//! of an mdBook. Code means a source or configuration blob ([`is_code_evidence`]): data and log
//! files, the audit trail and the subtrees the key space excludes name documents as data, so they
//! freeze nothing.

use std::collections::{HashMap, HashSet};

use crate::librarian::identity::key::{components, folder_of};
use crate::librarian::identity::key_space::KeySpace;
/// `HEAD`'s non-markdown text blobs, reduced to the markdown paths they name, and the mdBook
/// source trees of `HEAD`.
pub struct CodeRefs {
    /// A normalised repository path to the places (`blob path`, line) whose text names it.
    named: HashMap<String, Vec<(String, u32)>>,
    /// `(normalised src directory, path of the book.toml that names it)`.
    book_srcs: Vec<(String, String)>,
}

impl CodeRefs {
    /// Read the non-markdown blobs of `commit`: those that are UTF-8 with no NUL byte. A symlink
    /// and a submodule are not blobs of text and are skipped. Only a blob that
    /// [`is_code_evidence`] accepts is searched for the paths it names; every `book.toml` names an
    /// mdBook source tree.
    pub fn load(
        repo: &git2::Repository,
        commit: &git2::Commit,
        space: &KeySpace,
    ) -> anyhow::Result<CodeRefs> {
        let tree = commit.tree()?;
        let mut named: HashMap<String, Vec<(String, u32)>> = HashMap::new();
        let mut book_srcs = Vec::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            let is_blob = entry.kind() == Some(git2::ObjectType::Blob);
            let Some(name) = entry
                .name()
                .filter(|_| is_blob && entry.filemode() != LINK_MODE)
            else {
                return git2::TreeWalkResult::Ok;
            };
            let path = format!("{dir}{name}");
            if is_markdown(&path) {
                return git2::TreeWalkResult::Ok;
            }
            let evidence = is_code_evidence(&path, space);
            if !evidence && name != "book.toml" {
                return git2::TreeWalkResult::Ok;
            }
            let Ok(blob) = repo.find_blob(entry.id()) else {
                return git2::TreeWalkResult::Ok;
            };
            let bytes = blob.content();
            if bytes.contains(&0) {
                return git2::TreeWalkResult::Ok;
            }
            let Ok(text) = std::str::from_utf8(bytes) else {
                return git2::TreeWalkResult::Ok;
            };
            if name == "book.toml" {
                book_srcs.push((book_src_dir(&path, text), path.clone()));
            }
            if evidence {
                note_markdown_paths(&mut named, &path, text);
            }
            git2::TreeWalkResult::Ok
        })?;
        Ok(CodeRefs { named, book_srcs })
    }

    /// `path:line` of every non-markdown blob holding `needle`, sorted. A blob holds a path when
    /// it names it as written from the repository root, as a tail of a longer path, or relative to
    /// the blob's own folder (`./x.md`, `../docs/x.md`, `sub/x.md`).
    pub fn mentions(&self, needle: &str) -> Vec<String> {
        let mut found: Vec<String> = self
            .named
            .get(needle)
            .map(|hits| {
                hits.iter()
                    .map(|(path, line)| format!("{path}:{line}"))
                    .collect()
            })
            .unwrap_or_default();
        found.sort();
        found.dedup();
        found
    }

    /// D-R26: `Some(why)` when the markdown file at `rel_path` is frozen: it lies under a dot
    /// directory, it is a page of an mdBook, or code of `HEAD` names its path.
    pub fn frozen(&self, rel_path: &str) -> Option<String> {
        let parts = components(rel_path);
        let normal = parts.join("/");
        let dirs = &parts[..parts.len().saturating_sub(1)];
        if let Some(dot) = dirs.iter().find(|c| c.starts_with('.')) {
            return Some(format!(
                "it lies under the directory {dot}, whose files other tools read by name"
            ));
        }
        for (src, book_toml) in &self.book_srcs {
            if src.is_empty() || normal.starts_with(&format!("{src}/")) {
                return Some(format!(
                    "mdBook reads it: {book_toml} names its folder as the book source"
                ));
            }
        }
        let hits = self.mentions(&normal);
        let first = hits.first()?;
        let own_folder = folder_of(&normal);
        let same_folder = first
            .rsplit_once(':')
            .is_some_and(|(path, _)| folder_of(path) == own_folder);
        let more = match hits.len() {
            1 => String::new(),
            n => format!(" and {} more", n - 1),
        };
        Some(if same_folder {
            format!("code in its folder names the file at {first}{more}")
        } else {
            format!("code names its path at {first}{more}")
        })
    }
}

/// The mode of a symlink tree entry.
pub(crate) const LINK_MODE: i32 = 0o120000;

fn is_markdown(path: &str) -> bool {
    path.len() >= 3 && path.as_bytes()[path.len() - 3..].eq_ignore_ascii_case(b".md")
}

/// The extensions (ASCII case ignored) of blobs that hold data or logs: eval manifests, evidence
/// records, lists and tables name documents as values, and no program reads the document at the
/// path they hold.
pub(crate) const DATA_EXTENSIONS: [&str; 5] = ["jsonl", "txt", "json", "csv", "log"];

/// The audit trail of the catalog: tracked records of past calls, each naming the document a call
/// touched.
pub(crate) const AUDIT_DIR: &str = ".codescout/audit/";

/// Whether the blob at `path` counts as code for D-R26: a source or configuration file (`.rs`,
/// `.py`, `.sh`, `.toml`, `.yaml` and every other extension), which a program reads or runs, so a
/// path it names may be opened at run time. A data or log file ([`DATA_EXTENSIONS`]), a record of
/// the audit trail ([`AUDIT_DIR`]) and a blob under a subtree the key space excludes are not code:
/// each names documents as data, and on this repository they named most of `docs/` (I1).
fn is_code_evidence(path: &str, space: &KeySpace) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let data = name
        .rsplit_once('.')
        .is_some_and(|(_, ext)| DATA_EXTENSIONS.iter().any(|d| ext.eq_ignore_ascii_case(d)));
    !data && !path.starts_with(AUDIT_DIR) && !space.is_outside(path)
}

/// The normalised source directory of the book that `book_toml` (a path) configures: `[book]
/// src`, default `src`, under the folder of the file. A file that is not TOML gives the default.
fn book_src_dir(book_toml: &str, text: &str) -> String {
    let src = text
        .parse::<toml::Table>()
        .ok()
        .and_then(|table| table.get("book")?.get("src")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| "src".to_owned());
    components(&format!("{}/{src}", folder_of(book_toml))).join("/")
}

/// A byte that can be part of a path written in code. Bytes of a multi-byte character count, so
/// a run never starts or ends inside one.
fn is_path_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/' | b'+' | b'@') || b >= 0x80
}

/// Index every run of path bytes in `text` that ends in `.md`, under each path it can name.
fn note_markdown_paths(named: &mut HashMap<String, Vec<(String, u32)>>, blob: &str, text: &str) {
    let bytes = text.as_bytes();
    let newlines: Vec<usize> = text.match_indices('\n').map(|(i, _)| i).collect();
    let folder = folder_of(blob);
    for (dot, _) in text.match_indices('.') {
        if !bytes
            .get(dot + 1..dot + 3)
            .is_some_and(|ext| ext.eq_ignore_ascii_case(b"md"))
        {
            continue;
        }
        // A run ends at `.md` when no path byte follows; one trailing full stop is a sentence's.
        let ends = match bytes.get(dot + 3).copied() {
            Some(b'.') => bytes.get(dot + 4).is_none_or(|&n| !is_path_byte(n)),
            Some(b) => !is_path_byte(b),
            None => true,
        };
        if !ends {
            continue;
        }
        let mut start = dot;
        while start > 0 && is_path_byte(bytes[start - 1]) {
            start -= 1;
        }
        if start == dot {
            continue;
        }
        let line = newlines.partition_point(|&n| n < start) as u32 + 1;
        for key in names_of(&folder, &text[start..dot + 3]) {
            named.entry(key).or_default().push((blob.to_owned(), line));
        }
    }
}

/// The repository paths that the text of the markdown file `blob` names (a link, or a path written
/// out), each with the lines that name it, in the sense of [`CodeRefs::mentions`]. The cutover asks
/// it about files outside the key space, which it reports and never rewrites.
pub(crate) fn paths_named_in(blob: &str, text: &str) -> HashMap<String, Vec<(String, u32)>> {
    let mut named = HashMap::new();
    note_markdown_paths(&mut named, blob, text);
    named
}

/// The repository paths a written `run` can name: each tail of it that starts at a component
/// (code may write `../docs/x.md` or `$ROOT/docs/x.md`), and the run resolved against the folder
/// of the blob that holds it.
fn names_of(blob_folder: &str, run: &str) -> HashSet<String> {
    let parts: Vec<&str> = run
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    let mut names: HashSet<String> = (0..parts.len())
        .filter(|&k| parts[k] != "..")
        .map(|k| parts[k..].join("/"))
        .collect();
    let mut stack: Vec<&str> = blob_folder.split('/').filter(|p| !p.is_empty()).collect();
    let mut inside = true;
    for part in &parts {
        if *part == ".." {
            inside &= stack.pop().is_some();
        } else {
            stack.push(part);
        }
    }
    if inside {
        names.insert(stack.join("/"));
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::derived::scan::open_commit;
    use crate::librarian::derived::testkit::{repo_with, FILE};

    fn refs_of(files: &[(&str, &[u8])]) -> CodeRefs {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let entries: Vec<(&str, u32, &[u8])> = files.iter().map(|&(p, b)| (p, FILE, b)).collect();
        let repo = repo_with(&root, &entries);
        let commit = open_commit(&repo, "HEAD").unwrap();
        let space =
            crate::librarian::derived::build::key_space_from_commit(&repo, &commit).unwrap();
        CodeRefs::load(&repo, &commit, &space).unwrap()
    }

    #[test]
    fn a_path_in_any_code_file_freezes_the_file() {
        let refs = refs_of(&[
            (
                "src/server.rs",
                b"fn a() {}\n\nconst P: &str = \"../docs/X_Y.md\";\n",
            ),
            ("docs/X_Y.md", b"# X\n"),
            ("docs/other.md", b"# O\n"),
        ]);
        let why = refs.frozen("docs/X_Y.md").expect("code names the path");
        assert!(why.contains("src/server.rs:3"), "{why}");
        assert_eq!(refs.mentions("docs/X_Y.md"), ["src/server.rs:3"]);
        assert_eq!(refs.frozen("docs/other.md"), None);
    }

    #[test]
    fn a_file_name_in_code_of_the_same_folder_freezes_the_file() {
        let refs = refs_of(&[
            ("src/a/render_prompt.md", b"# P\n"),
            (
                "src/a/mod.rs",
                b"const P: &str = include_str!(\"./render_prompt.md\");\n",
            ),
        ]);
        let why = refs
            .frozen("src/a/render_prompt.md")
            .expect("the code of its folder names it");
        assert!(why.contains("src/a/mod.rs:1"), "{why}");
    }

    #[test]
    fn a_file_name_in_code_of_another_folder_does_not_freeze() {
        let refs = refs_of(&[
            ("src/a/render_prompt.md", b"# P\n"),
            ("src/b/mod.rs", b"const P: &str = \"render_prompt.md\";\n"),
        ]);
        assert_eq!(refs.frozen("src/a/render_prompt.md"), None);
    }

    #[test]
    fn a_relative_path_in_code_of_a_parent_folder_freezes_the_file() {
        let refs = refs_of(&[
            ("src/a/prompts/p.md", b"# P\n"),
            (
                "src/a/mod.rs",
                b"const P: &str = include_str!(\"prompts/p.md\");\n",
            ),
        ]);
        assert!(refs.frozen("src/a/prompts/p.md").is_some());
    }

    #[test]
    fn a_dot_directory_file_is_frozen() {
        let refs = refs_of(&[(".github/ISSUE_TEMPLATE/bug_report.md", b"# Bug\n")]);
        let why = refs
            .frozen(".github/ISSUE_TEMPLATE/bug_report.md")
            .expect("a dot directory dictates its names");
        assert!(why.contains(".github"), "{why}");
        assert_eq!(refs.frozen("docs/github/x.md"), None);
    }

    #[test]
    fn an_mdbook_page_is_frozen() {
        let refs = refs_of(&[
            (
                "docs/manual/book.toml",
                b"[book]\ntitle = \"M\"\nsrc = \"src\"\n",
            ),
            ("docs/manual/src/SUMMARY.md", b"# S\n"),
            ("docs/manual/src/a/page.md", b"# P\n"),
            ("docs/manual/notes.md", b"# N\n"),
        ]);
        assert!(refs.frozen("docs/manual/src/SUMMARY.md").is_some());
        assert!(refs.frozen("docs/manual/src/a/page.md").is_some());
        assert_eq!(refs.frozen("docs/manual/notes.md"), None);

        // No `src` key: mdBook's default is `src`.
        let refs = refs_of(&[
            ("book/book.toml", b"[book]\ntitle = \"M\"\n"),
            ("book/src/x.md", b"# X\n"),
        ]);
        assert!(refs.frozen("book/src/x.md").is_some());

        // Another `src` names another tree.
        let refs = refs_of(&[
            ("book.toml", b"[book]\nsrc = \"pages\"\n"),
            ("pages/x.md", b"# X\n"),
            ("src/y.md", b"# Y\n"),
        ]);
        assert!(refs.frozen("pages/x.md").is_some());
        assert_eq!(refs.frozen("src/y.md"), None);
    }

    #[test]
    fn binary_and_markdown_blobs_are_not_read_as_code() {
        let refs = refs_of(&[
            ("docs/a.md", b"see docs/x.md\n"),
            ("blob.dat", b"docs/y.md\0\x01\x02"),
            ("latin.txt", b"docs/z.md \xff\xfe\n"),
            ("docs/x.md", b"# X\n"),
            ("docs/y.md", b"# Y\n"),
            ("docs/z.md", b"# Z\n"),
        ]);
        assert_eq!(refs.frozen("docs/x.md"), None, "a markdown blob is no code");
        assert_eq!(refs.frozen("docs/y.md"), None, "a NUL byte marks a binary");
        assert_eq!(refs.frozen("docs/z.md"), None, "invalid UTF-8 is no text");
    }

    /// I1: data, logs and the audit trail name documents without reading them by path. Only
    /// source files are evidence, so none of these freezes `docs/a.md`.
    #[test]
    fn data_logs_and_the_audit_trail_do_not_freeze_a_file() {
        for blob in [
            "evals/seed-manifest.jsonl",
            "evidence/run.json",
            "notes/list.txt",
            "evals/table.csv",
            "logs/run.log",
            ".codescout/audit/ripper-202611.yaml",
            "contrib/tool.rs",
        ] {
            let refs = refs_of(&[
                (".codescout/key-space.toml", b"exclude = [\"contrib\"]\n"),
                (blob, b"{\"path\": \"docs/a.md\"}\n"),
                ("docs/a.md", b"# A\n"),
            ]);
            assert_eq!(refs.frozen("docs/a.md"), None, "{blob} is no code evidence");
            assert!(refs.mentions("docs/a.md").is_empty(), "{blob}");
        }
    }

    #[test]
    fn a_source_file_still_freezes_a_file_it_names() {
        for blob in ["src/x.rs", "scripts/run.py", "tools/check.sh", "Cargo.toml"] {
            let refs = refs_of(&[
                (".codescout/key-space.toml", b"exclude = [\"contrib\"]\n"),
                (blob, b"P = \"docs/a.md\"\n"),
                ("docs/a.md", b"# A\n"),
            ]);
            let why = refs.frozen("docs/a.md").unwrap_or_else(|| panic!("{blob}"));
            assert!(why.contains(&format!("{blob}:1")), "{why}");
        }
        // The extension is read without case, so `RUN.JSONL` is data as well.
        let refs = refs_of(&[("RUN.JSONL", b"docs/a.md\n"), ("docs/a.md", b"# A\n")]);
        assert_eq!(refs.frozen("docs/a.md"), None);
    }
}
