//! File system helpers.

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Walk upward from `start` looking for a directory containing `marker`.
/// Returns the directory path if found.
pub fn find_ancestor_with(start: &Path, marker: &str) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join(marker).exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Auto-detect the project root by walking upward and returning the **nearest**
/// ancestor that contains any of `.codescout/`, `.git/`, `Cargo.toml`,
/// `pyproject.toml`, `package.json`, or `go.mod`.
///
/// Distance wins over marker kind: a `.git`/`.codescout` in a *distant* ancestor
/// (e.g. the user's home directory, or `%TEMP%`'s parents on Windows) must not
/// shadow a nearer language manifest that marks the actual project. When a single
/// directory holds several markers it is simply returned (the kind doesn't matter
/// once the nearest marked directory is found).
pub fn detect_project_root(from: &Path) -> Option<PathBuf> {
    let markers = [
        ".codescout",
        ".git",
        "Cargo.toml",
        "pyproject.toml",
        "package.json",
        "go.mod",
    ];
    let mut current = from.to_path_buf();
    loop {
        if markers.iter().any(|marker| current.join(marker).exists()) {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Read a file as UTF-8, returning an error with the path on failure.
pub fn read_utf8(path: &Path) -> Result<String> {
    std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", path.display(), e))
}

/// Atomic write: write to a sibling `.tmp` file then rename, so a crash or
/// disk-full condition mid-write can't leave the target in a corrupt state.
/// The target file must have a parent directory (true for all real paths).
///
/// Preserves the target file's Unix permissions (e.g. exec bit) across the
/// rename. Without this, editing a `*.sh` script would silently strip +x
/// because the freshly-created tmp file has default 0644 perms.
pub fn atomic_write(path: &Path, content: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;

    // Preserve original mode if the target already exists.
    #[cfg(unix)]
    if let Ok(meta) = std::fs::metadata(path) {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(mode));
    }

    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Write UTF-8 content to a file, creating parent directories as needed.
/// Uses atomic write-then-rename to prevent corruption on crash.
pub fn write_utf8(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    atomic_write(path, content)
        .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", path.display(), e))
}

/// Normalize a path to its forward-slash string form.
///
/// Always replaces `\` with `/`, on every platform — the catalog stores
/// path strings in forward-slash form (see `artifact::upsert`,
/// `artifact_id_from_abs`, every LIKE pattern in `librarian::catalog` /
/// `librarian::tools/*`), so reads and writes must agree regardless of
/// host OS. Used at the boundary between filesystem paths and string
/// representations stored in the catalog DB or returned in MCP responses.
///
/// Idempotent: `to_forward_slash(to_forward_slash(p)) == to_forward_slash(p)`.
///
/// Caveat: a Linux filename containing a literal backslash byte gets that
/// byte rewritten to `/` for the catalog string form. Backslash-in-name
/// is legal on POSIX but vanishingly rare in source-code repos and markdown
/// docs, which is the only content the catalog stores. The actual
/// filesystem operations use the raw `Path`, so this only affects string
/// matching against the catalog — not file IO.
pub fn to_forward_slash(p: &std::path::Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// A path string in forward-slash separator form, suitable for catalog
/// storage, hashing into IDs, and LIKE-pattern construction.
///
/// Constructed only via [`RepoPath::from_path`] (or the equivalent
/// `From<&Path>` / `From<&PathBuf>` impls). Each constructor routes through
/// [`to_forward_slash`], so the inner string is guaranteed to contain no
/// backslash byte regardless of host platform.
///
/// This is a *write/storage* type — for paths that will be persisted in the
/// catalog DB, hashed via `artifact_id_from_abs`, or matched against catalog
/// rows in LIKE patterns. Paths intended only for display in MCP responses or
/// human-readable logs can keep using [`std::path::Path::to_string_lossy`]
/// directly; the invariant carried by `RepoPath` is specifically about catalog
/// correctness.
///
/// `RepoPath` does not encode abs-vs-rel. Both forms appear in the catalog
/// (`artifact.abs_path` is absolute; `artifact_event.rel_path` is relative).
/// Callers that need to enforce one shape over the other should validate
/// separately — see [`librarian::tools::gather::guard_relative_path`] for the
/// relative-path validator.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoPath(String);

impl RepoPath {
    /// Build a `RepoPath` from any `&Path`, normalizing separators.
    pub fn from_path(p: &std::path::Path) -> Self {
        Self(to_forward_slash(p))
    }

    /// Borrow the inner string. Use this for `rusqlite::params!` and
    /// `format!` arguments where a `&str` is expected.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the `RepoPath`, returning the owned forward-slash string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for RepoPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RepoPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&std::path::Path> for RepoPath {
    fn from(p: &std::path::Path) -> Self {
        Self::from_path(p)
    }
}

impl From<&std::path::PathBuf> for RepoPath {
    fn from(p: &std::path::PathBuf) -> Self {
        Self::from_path(p.as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn find_ancestor_finds_marker_in_current_dir() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(
            find_ancestor_with(dir.path(), "Cargo.toml"),
            Some(dir.path().to_path_buf())
        );
    }

    #[test]
    fn find_ancestor_walks_up_to_parent() {
        let dir = tempdir().unwrap();
        let child = dir.path().join("src").join("nested");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(
            find_ancestor_with(&child, "Cargo.toml"),
            Some(dir.path().to_path_buf())
        );
    }

    #[test]
    fn find_ancestor_returns_none_when_absent() {
        let dir = tempdir().unwrap();
        assert_eq!(
            find_ancestor_with(dir.path(), "nonexistent-xyz-marker"),
            None
        );
    }

    #[test]
    fn detect_project_root_finds_cargo_toml() {
        let dir = tempdir().unwrap();
        let deep = dir.path().join("src").join("module");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(detect_project_root(&deep), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn detect_project_root_prefers_codescout_dir_over_git() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::create_dir(dir.path().join(".codescout")).unwrap();
        // .codescout takes priority (first in markers list)
        assert_eq!(
            detect_project_root(dir.path()),
            Some(dir.path().to_path_buf())
        );
    }

    #[test]
    fn read_write_utf8_roundtrip() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        write_utf8(&file, "hello world").unwrap();
        assert_eq!(read_utf8(&file).unwrap(), "hello world");
    }

    #[test]
    fn write_utf8_creates_intermediate_dirs() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("a").join("b").join("c.txt");
        write_utf8(&file, "deep content").unwrap();
        assert_eq!(read_utf8(&file).unwrap(), "deep content");
    }

    #[test]
    fn read_utf8_missing_file_errors() {
        let dir = tempdir().unwrap();
        assert!(read_utf8(&dir.path().join("missing.txt")).is_err());
    }

    #[test]
    fn to_forward_slash_converts_backslashes_on_any_platform() {
        let p = std::path::PathBuf::from("C:\\roots\\alive\\a.md");
        assert_eq!(to_forward_slash(&p), "C:/roots/alive/a.md");
    }

    #[test]
    fn to_forward_slash_passes_through_forward_slash_input() {
        let p = std::path::PathBuf::from("/already/forward/slash.md");
        assert_eq!(to_forward_slash(&p), "/already/forward/slash.md");
    }

    #[test]
    fn to_forward_slash_is_idempotent() {
        let p = std::path::PathBuf::from("C:\\mixed/separators\\foo.md");
        let once = to_forward_slash(&p);
        let twice = to_forward_slash(std::path::Path::new(&once));
        assert_eq!(once, twice);
        assert_eq!(once, "C:/mixed/separators/foo.md");
    }

    #[test]
    fn repo_path_from_path_normalizes_backslashes() {
        let p = std::path::PathBuf::from("C:\\roots\\alive\\a.md");
        let rp = RepoPath::from_path(&p);
        assert_eq!(rp.as_str(), "C:/roots/alive/a.md");
    }

    #[test]
    fn repo_path_from_trait_works_for_path_and_pathbuf() {
        let pb = std::path::PathBuf::from("a\\b\\c.md");
        let from_pb: RepoPath = RepoPath::from(&pb);
        let from_path: RepoPath = RepoPath::from(pb.as_path());
        assert_eq!(from_pb, from_path);
        assert_eq!(from_pb.as_str(), "a/b/c.md");
    }

    #[test]
    fn repo_path_display_matches_inner() {
        let rp = RepoPath::from_path(std::path::Path::new("foo\\bar"));
        assert_eq!(format!("{}", rp), "foo/bar");
        assert_eq!(format!("{}/%", rp), "foo/bar/%");
    }

    #[test]
    fn repo_path_as_ref_str_works_with_format_args() {
        let rp = RepoPath::from_path(std::path::Path::new("docs\\foo.md"));
        let s: &str = rp.as_ref();
        assert_eq!(s, "docs/foo.md");
    }

    #[test]
    fn repo_path_idempotent_via_string_roundtrip() {
        let p = std::path::PathBuf::from("C:\\mixed/seps\\foo.md");
        let once = RepoPath::from_path(&p);
        let twice = RepoPath::from_path(std::path::Path::new(once.as_str()));
        assert_eq!(once, twice);
        assert_eq!(once.as_str(), "C:/mixed/seps/foo.md");
    }

    #[test]
    fn repo_path_into_string_consumes() {
        let rp = RepoPath::from_path(std::path::Path::new("a\\b"));
        let owned: String = rp.into_string();
        assert_eq!(owned, "a/b");
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_exec_bit() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("script.sh");
        std::fs::write(&path, "#!/bin/sh\necho old\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();

        atomic_write(&path, "#!/bin/sh\necho new\n").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "exec bit must survive atomic_write");
    }
}
