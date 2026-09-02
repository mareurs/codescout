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

/// The staging path `atomic_write` writes through before renaming onto `path`.
///
/// Appends to the **whole filename** rather than replacing the extension, and that
/// distinction is the only reason this helper exists. `Path::with_extension("tmp")` — what
/// this used to be — derives the staging path from the directory and stem alone, so
/// `Cargo.toml` and `Cargo.lock` both stage through `Cargo.tmp`. Two concurrent writers then
/// rename *each other's* content onto their own targets: cross-file corruption rather than a
/// lost update, with a successful return on both sides and nothing logged anywhere.
///
/// Measured 2026-08-31 from `git ls-files`: **7** groups of tracked files in this repo share
/// a stem, including `Cargo.toml`/`Cargo.lock`, five `.env.*` variants, and
/// `src/prompts/source.md`/`source.rs` — the last a prompt surface with three consumers that
/// must stay consistent, written through `edit_file` and `edit_code`, both of which route
/// here.
/// `docs/issues/archive/2026-08-31-atomic-write-tmp-path-collides-across-same-stem-files.md`.
///
/// **This does not make `atomic_write` race-free and must not be read as doing so.** Two
/// writers to the *same* target still race — inherent to write-then-rename without a lock.
/// What this removes is only the case where the damage lands on a file the caller never
/// named. A per-writer unique suffix would go further and is a separate decision.
///
/// The sibling `with_extension` uses at `src/lsp/mux/mod.rs` and `src/retrieval/index_lock.rs`
/// are **not** instances of this defect: both take an internally-built `*.lock` path, so their
/// input namespace holds exactly one extension and no two inputs can share a stem. Checked
/// rather than assumed, because a fix that names a population asserts that population is
/// non-empty.
fn staging_path(path: &Path) -> PathBuf {
    match path.file_name() {
        Some(name) => {
            let mut staged = name.to_os_string();
            staged.push(".tmp");
            path.with_file_name(staged)
        }
        // A path with no final component — a bare root, or one ending in `..`. No caller can
        // reach this (all 15 call sites pass a real file path) and `atomic_write` on such a
        // path cannot succeed anyway, since the rename target is not a file. Deliberately
        // preserves the previous derivation rather than synthesising a name: appending to an
        // empty filename would yield a writable `<root>/.tmp`, turning an operation that used
        // to fail harmlessly into one that creates a stray file at the filesystem root.
        None => path.with_extension("tmp"),
    }
}

/// Atomic write: write to a sibling `.tmp` file then rename, so a crash or
/// disk-full condition mid-write can't leave the target in a corrupt state.
/// The target file must have a parent directory (true for all real paths).
///
/// Preserves the target file's Unix permissions (e.g. exec bit) across the
/// rename. Without this, editing a `*.sh` script would silently strip +x
/// because the freshly-created tmp file has default 0644 perms.
pub fn atomic_write(path: &Path, content: &str) -> std::io::Result<()> {
    let tmp = staging_path(path);

    // Cleanup is needed on BOTH failure points, not just the rename below.
    // `std::fs::write` is `File::create` + `write_all`, so under ENOSPC the create
    // can SUCCEED — the tmp file now exists — while the write fails. Before this
    // guard the `?` propagated straight out and left the sibling `.tmp` behind;
    // the target itself was always safe, because only the rename touches it.
    std::fs::write(&tmp, content).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;

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

/// Per-user directory for **persistent** state.
///
/// Distinct from [`crate::socket_discovery::per_user_runtime_dir`], which serves
/// sockets and lock files that are expected to die with the boot. This one holds
/// data that must outlive a reboot.
///
/// `$XDG_STATE_HOME` when set to an absolute path, else `$HOME/.local/state`.
/// `None` when neither is available — callers degrade rather than guess.
pub fn per_user_state_dir() -> Option<PathBuf> {
    state_dir_from(
        std::env::var_os("XDG_STATE_HOME"),
        crate::platform::home_dir(),
    )
}

/// Pure core of [`per_user_state_dir`], split out so tests never mutate the
/// process environment — concurrent `set_var` is UB and this suite is not
/// serialized (docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md).
fn state_dir_from(xdg: Option<std::ffi::OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(x) = xdg {
        let p = PathBuf::from(x);
        // The XDG basedir spec: a relative value must be treated as unset.
        if p.is_absolute() {
            return Some(p);
        }
    }
    Some(home?.join(".local").join("state"))
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

/// Render `path` relative to `root` in forward-slash form, falling back to
/// the absolute forward-slash form when `path` is not under `root`.
///
/// The `path.strip_prefix(root).unwrap_or(path)` + stringify idiom repeats
/// across every tool that reports a file path relative to a project or
/// library root (`list_overview`, `classify_reference_path`, `symbol_at`,
/// `agent` status, ...) — this is the one place it lives. Backslash-safety
/// on Windows is inherited from [`to_forward_slash`]; `strip_prefix` itself
/// is a `std::path::Path` component operation and already parses `\` vs `/`
/// correctly per host platform.
pub fn relative_forward_slash(path: &std::path::Path, root: &std::path::Path) -> String {
    to_forward_slash(path.strip_prefix(root).unwrap_or(path))
}

/// Length in bytes of a Windows drive-letter prefix (`C:`) at the start of
/// `s`, including the `//?/` extended-length "verbatim" marker that
/// `fs::canonicalize` prepends on Windows (stored here in forward-slash
/// form by [`to_forward_slash`] — e.g. `//?/C:/Users/...`). Returns `None`
/// if `s` has no drive-letter prefix in either form.
///
/// Detects the string *shape*, not the host platform: catalog `abs_path`
/// values may have been captured on a different OS than the one running
/// the check, so this must not be `cfg(windows)`-gated.
pub fn drive_letter_prefix_len(s: &str) -> Option<usize> {
    let verbatim_len = if s.starts_with("//?/") { 4 } else { 0 };
    let rest = s.as_bytes().get(verbatim_len..)?;
    (rest.len() >= 2 && rest[0].is_ascii_alphabetic() && rest[1] == b':')
        .then_some(verbatim_len + 2)
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
    fn relative_forward_slash_strips_root() {
        let root = std::path::PathBuf::from("/proj");
        let path = std::path::PathBuf::from("/proj/src/lib.rs");
        assert_eq!(relative_forward_slash(&path, &root), "src/lib.rs");
    }

    #[test]
    fn relative_forward_slash_falls_back_to_absolute_outside_root() {
        let root = std::path::PathBuf::from("/proj");
        let path = std::path::PathBuf::from("/other/lib.rs");
        assert_eq!(relative_forward_slash(&path, &root), "/other/lib.rs");
    }

    #[test]
    fn drive_letter_prefix_len_bare_drive() {
        assert_eq!(drive_letter_prefix_len("C:/Users/x/foo.md"), Some(2));
        assert_eq!(drive_letter_prefix_len("z:/"), Some(2));
        assert_eq!(drive_letter_prefix_len("C:foo.txt"), Some(2));
    }

    #[test]
    fn drive_letter_prefix_len_verbatim_form() {
        assert_eq!(
            drive_letter_prefix_len("//?/C:/Users/marius/foo.md"),
            Some(6)
        );
        assert_eq!(drive_letter_prefix_len("//?/C:/foo.txt:stream"), Some(6));
    }

    #[test]
    fn drive_letter_prefix_len_none_for_posix_and_malformed() {
        assert_eq!(drive_letter_prefix_len("/home/marius/foo.md"), None);
        assert_eq!(drive_letter_prefix_len("docs/foo.md"), None);
        assert_eq!(drive_letter_prefix_len("Cusers/foo.md"), None);
        assert_eq!(drive_letter_prefix_len(""), None);
        assert_eq!(drive_letter_prefix_len("//?/"), None);
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

    /// The staging path must be unique per TARGET FILENAME, not per stem.
    ///
    /// `Path::with_extension("tmp")` — what this derivation used to be — builds the staging
    /// path from the directory and stem alone, discarding the one component that tells
    /// `Cargo.toml` from `Cargo.lock`. Two concurrent `atomic_write`s to same-stem files then
    /// stage through one path, and the loser does not merely lose its own write: it renames
    /// the OTHER file's content onto its own target. Cross-file corruption, and both callers
    /// are told the write succeeded.
    ///
    /// **Every group below is a real collision group in this repo**, enumerated from
    /// `git ls-files` on 2026-08-31 — they are findings, not illustrations, and replacing them
    /// with invented names would leave the test passing while it stopped describing anything.
    /// `source.md`/`source.rs` is the pair that sets the severity: `CLAUDE.md` § *Prompt
    /// Surface Consistency* makes `source.md` a load-bearing surface with three consumers that
    /// must stay consistent, and `edit_file` and `edit_code` both route through
    /// `atomic_write`. The extensionless group is here because it was **not** anticipated when
    /// the bug was filed; drop it and a fix that special-cases only dotted filenames still
    /// passes this test.
    ///
    /// Asserts DISTINCTNESS rather than a literal name deliberately. A literal pins the suffix
    /// scheme and would need rewriting for any later change to it, while distinctness is the
    /// contract that actually protects the caller.
    #[test]
    fn staging_paths_are_distinct_for_files_that_share_a_stem() {
        let dir = Path::new("/repo");
        let groups: [&[&str]; 4] = [
            &["Cargo.toml", "Cargo.lock"],
            &["source.md", "source.rs"],
            &[
                ".env.amd",
                ".env.cpu",
                ".env.example",
                ".env.gpu",
                ".env.lite",
            ],
            &["target", "target.md"],
        ];

        for group in groups {
            let mut seen: Vec<(&str, PathBuf)> = Vec::new();
            for name in group {
                let staged = staging_path(&dir.join(name));
                if let Some((other, _)) = seen.iter().find(|(_, p)| *p == staged) {
                    panic!(
                        "'{name}' and '{other}' both stage through {} — a concurrent write to \
                         either renames the other's content onto its own target",
                        staged.display()
                    );
                }
                seen.push((name, staged));
            }
        }
    }

    /// The staging path must stay a SIBLING of its target.
    ///
    /// `atomic_write` finishes with `std::fs::rename`, which fails across filesystems. Staging
    /// anywhere but the target's own directory — a temp dir, say — would turn every write into
    /// an EXDEV error the moment the repo and `/tmp` sit on different mounts, and it is the
    /// sibling placement that makes the rename atomic at all. Pinned separately from the
    /// distinctness test above because uniqueness is trivially satisfiable by moving the file:
    /// a fix that bought distinctness with a process-unique path in `/tmp` would pass that test
    /// and break every write.
    #[test]
    fn the_staging_path_stays_beside_its_target() {
        let target = Path::new("/repo/src/prompts/source.md");
        let staged = staging_path(target);

        assert_eq!(
            staged.parent(),
            target.parent(),
            "staging must happen in the target's own directory, or the final rename crosses \
             filesystems: {} vs {}",
            staged.display(),
            target.display()
        );
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

    /// The leak the `rename` guard never covered. `std::fs::write` is
    /// `File::create` + `write_all`, so under ENOSPC the create SUCCEEDS — the tmp
    /// file exists from that moment — and the write then fails. The `?` propagated
    /// straight out, leaving the sibling `.tmp` behind.
    ///
    /// **Reproducing it requires create to succeed and the write to fail**, and
    /// that is the whole difficulty. The obvious reproductions — making the tmp
    /// path a directory, or its parent read-only — fail at `File::create` instead,
    /// so nothing is created, nothing leaks, and such a test PASSES against the
    /// unfixed function. `/dev/full` gives the right shape: the open succeeds and
    /// every write returns ENOSPC. Verified at the shell before this was written,
    /// then demonstrated failing against the unfixed function.
    ///
    /// Linux-only: `/dev/full` does not exist on macOS, and CI runs a macOS lane.
    #[cfg(target_os = "linux")]
    #[test]
    fn atomic_write_removes_its_tmp_file_when_the_write_itself_fails() {
        // Absence of /dev/full must be LOUD. A graceful skip here would turn this
        // into a clean `0 passed` that is character-identical to coverage.
        assert!(
            Path::new("/dev/full").exists(),
            "/dev/full is missing, so this test cannot reproduce ENOSPC — that is a \
             broken harness, not a passing test"
        );

        let dir = tempdir().unwrap();
        let target = dir.path().join("target");
        let tmp = target.with_extension("tmp");

        // Pre-existing content, so the atomicity guarantee is pinned as "unchanged"
        // rather than merely "not created" — the stronger of the two properties, and
        // the one the .tmp-then-rename dance exists for.
        const ORIGINAL: &str = "original content, must survive a failed write\n";
        std::fs::write(&target, ORIGINAL).unwrap();

        std::os::unix::fs::symlink("/dev/full", &tmp).unwrap();

        // (1) It must FAIL. A version that swallowed the error and returned Ok would
        // satisfy the cleanup assertion below while breaking the function's purpose.
        let err = atomic_write(&target, "new content")
            .expect_err("atomic_write must propagate the write failure, not swallow it");

        // (2) Positive control: pin that we reproduced the INTENDED failure. Without
        // it, a mechanism that silently started yielding some other error (EACCES,
        // say) would still satisfy (3) and (4) while exercising a different path.
        assert_eq!(
            err.raw_os_error(),
            Some(28),
            "expected ENOSPC (28) from /dev/full, got {err:?}"
        );

        // (3) Atomicity: the target is untouched, because only the rename ever
        // touches it and the failure happened well before that.
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            ORIGINAL,
            "a failed write must leave the target byte-identical"
        );

        // (4) The leak itself. `symlink_metadata`, not `exists()`: `exists()` follows
        // the link and would report on /dev/full — which always exists — rather than
        // on the tmp entry that must have been removed.
        assert!(
            tmp.symlink_metadata().is_err(),
            "atomic_write leaked {tmp:?} after the write failed"
        );
    }

    // `Path::is_absolute()` requires a drive or UNC prefix on Windows — a bare
    // leading `/` has a root but no prefix, so it is NOT absolute there. Use a
    // per-platform literal for every state_dir_* test that needs an absolute
    // XDG value, on both the input and the expected output.
    const ABS_XDG: &str = if cfg!(windows) {
        r"C:\xdg\state"
    } else {
        "/xdg/state"
    };

    #[test]
    fn state_dir_prefers_an_absolute_xdg_state_home() {
        let got = state_dir_from(
            Some(std::ffi::OsString::from(ABS_XDG)),
            Some(PathBuf::from("/home/u")),
        );
        assert_eq!(got, Some(PathBuf::from(ABS_XDG)));
    }

    #[test]
    fn state_dir_ignores_a_relative_xdg_state_home() {
        // The XDG basedir spec requires relative paths to be treated as unset.
        let got = state_dir_from(
            Some(std::ffi::OsString::from("relative/state")),
            Some(PathBuf::from("/home/u")),
        );
        assert_eq!(got, Some(PathBuf::from("/home/u/.local/state")));
    }

    #[test]
    fn state_dir_falls_back_to_home_local_state() {
        let got = state_dir_from(None, Some(PathBuf::from("/home/u")));
        assert_eq!(got, Some(PathBuf::from("/home/u/.local/state")));
    }

    #[test]
    fn state_dir_is_none_when_neither_is_available() {
        // The caller degrades to an in-memory ledger rather than guessing a path.
        assert_eq!(state_dir_from(None, None), None);
    }

    #[test]
    fn state_dir_uses_xdg_even_when_home_is_absent() {
        // An absolute XDG_STATE_HOME must not be discarded just because
        // HOME/USERPROFILE is unset (systemd user units, containers).
        let got = state_dir_from(Some(std::ffi::OsString::from(ABS_XDG)), None);
        assert_eq!(got, Some(PathBuf::from(ABS_XDG)));
    }

    #[test]
    fn state_dir_is_none_when_xdg_relative_and_home_absent() {
        let got = state_dir_from(Some(std::ffi::OsString::from("relative/state")), None);
        assert_eq!(got, None);
    }
}
