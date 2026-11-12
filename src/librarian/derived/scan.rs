//! Scan one commit's tree for markdown blobs, through `git2` only.
//!
//! Nothing here touches the working tree: the derived index describes a commit, so a dirty
//! checkout cannot change it. Every blob is read from the object database.

use anyhow::Context;
use sha2::{Digest, Sha256};

use crate::librarian::identity::key_space::KeySpace;

/// Why a markdown blob has no text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// The bytes are not valid UTF-8.
    NotUtf8,
    /// The tree entry is a symlink (mode 120000).
    Symlink,
    /// The key space excludes the path.
    OutsideKeySpace,
}

impl Skip {
    /// The stable reason string recorded for a skipped file.
    pub fn reason(&self) -> &'static str {
        match self {
            Skip::NotUtf8 => "not_utf8",
            Skip::Symlink => "symlink",
            Skip::OutsideKeySpace => "outside_key_space",
        }
    }
}

/// One `.md` blob of a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFile {
    /// Git-root-relative, `/`-separated.
    pub rel_path: String,
    /// Lower-case hex SHA-256 of the raw blob bytes.
    pub sha256: String,
    /// The text, or why there is none.
    pub text: Result<String, Skip>,
}

/// Resolve `rev` (`HEAD` or a sha) to a commit.
pub fn open_commit<'r>(repo: &'r git2::Repository, rev: &str) -> anyhow::Result<git2::Commit<'r>> {
    repo.revparse_single(rev)
        .and_then(|obj| obj.peel_to_commit())
        .with_context(|| format!("resolving {rev:?} to a commit"))
}

/// The UTF-8 text of `rel_path` in `commit`'s tree. `None` when the path is absent; an error
/// when the blob is not UTF-8.
pub fn blob_text(
    repo: &git2::Repository,
    commit: &git2::Commit,
    rel_path: &str,
) -> anyhow::Result<Option<String>> {
    let entry = match commit.tree()?.get_path(std::path::Path::new(rel_path)) {
        Ok(entry) => entry,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => {
            return Err(e).with_context(|| format!("looking up {rel_path} in {}", commit.id()))
        }
    };
    let blob = repo
        .find_blob(entry.id())
        .with_context(|| format!("{rel_path} in {} is not a file", commit.id()))?;
    let text = std::str::from_utf8(blob.content())
        .with_context(|| format!("{rel_path} in {} is not UTF-8", commit.id()))?;
    Ok(Some(text.to_string()))
}

/// Every `.md` blob in `commit`'s tree (extension compared case-sensitively), sorted by path.
pub fn scan_commit(
    repo: &git2::Repository,
    commit: &git2::Commit,
    space: &KeySpace,
) -> anyhow::Result<Vec<ScannedFile>> {
    let mut files = Vec::new();
    collect(repo, &commit.tree()?, "", space, &mut files)?;
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(files)
}

/// The mode of a symlink tree entry.
const SYMLINK_MODE: i32 = 0o120000;

fn collect(
    repo: &git2::Repository,
    tree: &git2::Tree,
    prefix: &str,
    space: &KeySpace,
    out: &mut Vec<ScannedFile>,
) -> anyhow::Result<()> {
    for entry in tree.iter() {
        // A path that is not UTF-8 cannot be a `rel_path`. Its lossy spelling is kept so the
        // file is still reported, as `NotUtf8`, instead of vanishing.
        let (name, name_ok) = match std::str::from_utf8(entry.name_bytes()) {
            Ok(n) => (n.to_string(), true),
            Err(_) => (
                String::from_utf8_lossy(entry.name_bytes()).into_owned(),
                false,
            ),
        };
        let rel_path = format!("{prefix}{name}");
        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                let sub = repo
                    .find_tree(entry.id())
                    .with_context(|| format!("reading tree {rel_path}"))?;
                collect(repo, &sub, &format!("{rel_path}/"), space, out)?;
            }
            // Submodules (commit entries) have no blob here.
            Some(git2::ObjectType::Blob) if name.ends_with(".md") => {
                let blob = repo
                    .find_blob(entry.id())
                    .with_context(|| format!("reading blob {rel_path}"))?;
                let bytes = blob.content();
                let sha256 = format!("{:x}", Sha256::digest(bytes));
                // An excluded path's bytes are never decoded.
                let text = if space.is_outside(&rel_path) {
                    Err(Skip::OutsideKeySpace)
                } else if entry.filemode() == SYMLINK_MODE {
                    Err(Skip::Symlink)
                } else if !name_ok {
                    Err(Skip::NotUtf8)
                } else {
                    std::str::from_utf8(bytes)
                        .map(str::to_string)
                        .map_err(|_| Skip::NotUtf8)
                };
                out.push(ScannedFile {
                    rel_path,
                    sha256,
                    text,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed author time, so a commit id depends on the content alone.
    fn fixed_sig() -> git2::Signature<'static> {
        git2::Signature::new(
            "Test",
            "test@example.invalid",
            &git2::Time::new(1_700_000_000, 0),
        )
        .unwrap()
    }

    /// A repo in `dir` with one commit holding `entries` (`path`, `mode`, `bytes`). Built through
    /// the index and an explicit signature: no working tree, no global git config.
    fn repo_with(dir: &std::path::Path, entries: &[(&str, u32, &[u8])]) -> git2::Repository {
        let repo = git2::Repository::init(dir).unwrap();
        // The repository's own index, used only to assemble a tree: it is never written to disk.
        let mut index = repo.index().unwrap();
        for &(path, mode, bytes) in entries {
            let entry = git2::IndexEntry {
                ctime: git2::IndexTime::new(0, 0),
                mtime: git2::IndexTime::new(0, 0),
                dev: 0,
                ino: 0,
                mode,
                uid: 0,
                gid: 0,
                file_size: bytes.len() as u32,
                id: git2::Oid::zero(),
                flags: 0,
                flags_extended: 0,
                path: path.as_bytes().to_vec(),
            };
            index.add_frombuffer(&entry, bytes).unwrap();
        }
        let tree_id = index.write_tree().unwrap();
        {
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = fixed_sig();
            repo.commit(Some("HEAD"), &sig, &sig, "fixture", &tree, &[])
                .unwrap();
        }
        repo
    }

    const FILE: u32 = 0o100644;
    const LINK: u32 = 0o120000;

    fn scan(repo: &git2::Repository, space: &KeySpace) -> Vec<ScannedFile> {
        let commit = open_commit(repo, "HEAD").unwrap();
        scan_commit(repo, &commit, space).unwrap()
    }

    #[test]
    fn lists_only_md_blobs_sorted_by_path() {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with(
            dir.path(),
            &[
                ("z.md", FILE, b"z"),
                ("docs/b.md", FILE, b"b"),
                ("docs/a.md", FILE, b"a"),
                ("src/main.rs", FILE, b"fn main() {}"),
                ("README.txt", FILE, b"txt"),
                ("a.md", FILE, b"root a"),
            ],
        );
        let files = scan(&repo, &KeySpace::empty());
        let paths: Vec<&str> = files.iter().map(|f| f.rel_path.as_str()).collect();
        assert_eq!(paths, ["a.md", "docs/a.md", "docs/b.md", "z.md"]);
        assert_eq!(files[1].text, Ok("a".to_string()));
    }

    #[test]
    fn a_non_utf8_file_is_skipped_with_a_reason() {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with(
            dir.path(),
            &[
                ("bad.md", FILE, &[0x66, 0xff, 0xfe, 0x6f]),
                ("ok.md", FILE, b"fine"),
            ],
        );
        let files = scan(&repo, &KeySpace::empty());
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].rel_path, "bad.md");
        // Strict decoding: no U+FFFD substitution, the file has no text at all.
        assert_eq!(files[0].text, Err(Skip::NotUtf8));
        assert_eq!(files[0].text.as_ref().unwrap_err().reason(), "not_utf8");
        assert_eq!(files[1].text, Ok("fine".to_string()));
    }

    #[test]
    fn a_symlink_entry_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with(
            dir.path(),
            &[("link.md", LINK, b"real.md"), ("real.md", FILE, b"body")],
        );
        let files = scan(&repo, &KeySpace::empty());
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].rel_path, "link.md");
        assert_eq!(files[0].text, Err(Skip::Symlink));
        assert_eq!(files[0].text.as_ref().unwrap_err().reason(), "symlink");
        assert_eq!(files[1].text, Ok("body".to_string()));
    }

    #[test]
    fn an_excluded_path_is_listed_but_its_text_is_not_read() {
        let dir = tempfile::tempdir().unwrap();
        // The excluded file is not UTF-8: had its text been decoded, the skip would be
        // `NotUtf8`, not `OutsideKeySpace`.
        let repo = repo_with(
            dir.path(),
            &[
                ("contrib/pi/AGENTS.md", FILE, &[0xff, 0xfe]),
                ("docs/a.md", FILE, b"a"),
            ],
        );
        let space = KeySpace::from_patterns(&["contrib"]).unwrap();
        let files = scan(&repo, &space);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].rel_path, "contrib/pi/AGENTS.md");
        assert_eq!(files[0].text, Err(Skip::OutsideKeySpace));
        assert_eq!(
            files[0].text.as_ref().unwrap_err().reason(),
            "outside_key_space"
        );
        assert_eq!(files[1].text, Ok("a".to_string()));
    }

    #[test]
    fn sha256_matches_the_old_walks_hex_form() {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with(dir.path(), &[("a.md", FILE, b"hello\n")]);
        let files = scan(&repo, &KeySpace::empty());
        // `printf 'hello\n' | sha256sum`: 64 lower-case hex digits over the raw bytes, the form
        // `indexer.rs` stores in `file_sha256`.
        assert_eq!(
            files[0].sha256,
            "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03"
        );
    }

    #[test]
    fn an_uppercase_md_extension_is_not_listed() {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with(
            dir.path(),
            &[
                ("NOTES.MD", FILE, b"x"),
                ("Notes.Md", FILE, b"x"),
                ("n.md", FILE, b"x"),
            ],
        );
        let files = scan(&repo, &KeySpace::empty());
        let paths: Vec<&str> = files.iter().map(|f| f.rel_path.as_str()).collect();
        assert_eq!(paths, ["n.md"]);
    }

    #[test]
    fn an_empty_repo_with_no_commit_is_an_error_naming_head() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let err = open_commit(&repo, "HEAD").unwrap_err();
        assert!(format!("{err:#}").contains("HEAD"), "got: {err:#}");
    }

    #[test]
    fn blob_text_returns_none_for_an_absent_path_and_an_error_for_non_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with(
            dir.path(),
            &[
                (".codescout/repo.toml", FILE, b"id = 'x'\n"),
                ("bad.toml", FILE, &[0xff, 0xfe]),
            ],
        );
        let commit = open_commit(&repo, "HEAD").unwrap();
        assert_eq!(
            blob_text(&repo, &commit, ".codescout/repo.toml").unwrap(),
            Some("id = 'x'\n".to_string())
        );
        assert_eq!(blob_text(&repo, &commit, "absent.toml").unwrap(), None);
        let err = blob_text(&repo, &commit, "bad.toml").unwrap_err();
        assert!(format!("{err:#}").contains("bad.toml"), "got: {err:#}");
    }

    #[test]
    fn open_commit_accepts_a_sha() {
        let dir = tempfile::tempdir().unwrap();
        let repo = repo_with(dir.path(), &[("a.md", FILE, b"a")]);
        let head = open_commit(&repo, "HEAD").unwrap().id();
        let by_sha = open_commit(&repo, &head.to_string()).unwrap();
        assert_eq!(by_sha.id(), head);
    }

    #[test]
    fn commit_ids_are_reproducible() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let ra = repo_with(a.path(), &[("a.md", FILE, b"a")]);
        let rb = repo_with(b.path(), &[("a.md", FILE, b"a")]);
        assert_eq!(
            open_commit(&ra, "HEAD").unwrap().id(),
            open_commit(&rb, "HEAD").unwrap().id()
        );
    }
}
