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

/// The most bytes a configuration file (`project.toml`, the global `config.toml`, `workspace.toml`,
/// the startup dotenv) may hold before it is refused. The config loaders named in
/// `docs/issues/2026-10-19-the-config-loaders-read-a-linked-special-file-without-a-bound.md` read
/// through [`read_regular_config_bytes`] / [`read_regular_config_text`], so there is one number;
/// the reads that remain outside it are listed in
/// `docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md`.
// cap-class: NOT_A_CAP — bounds how many bytes of an INPUT configuration file are read; a file over it is refused whole with an error, no result is shortened and nothing is presented as complete
pub const MAX_CONFIG_FILE_BYTES: u64 = 1024 * 1024;

/// Why [`read_regular_bytes`] refused. `Display` names the reason only, never the file's content
/// or its path: the file can be a link to anything readable, and the message reaches the
/// caller, the log and the usage row. The caller adds the path.
#[derive(Debug)]
pub enum BoundedReadError {
    /// The opened descriptor is not a regular file: a directory, FIFO, socket or device. It is
    /// never read, so a link to `/dev/zero` or to a FIFO nobody writes to cannot hang the caller.
    NotRegular,
    /// More than `max` bytes. The bound is on the bytes actually read, not on the length the file
    /// reports, so a file that lies about its size (`/proc`, a file growing while read) is still cut.
    TooLarge { max: u64 },
    /// The open or the read failed, or the bytes were not UTF-8 (text reads only).
    Io(std::io::Error),
}

impl BoundedReadError {
    /// The path does not exist (also: a dangling link). The one `Io` case callers treat as
    /// "absent" rather than "broken".
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::Io(e) if e.kind() == std::io::ErrorKind::NotFound)
    }
}

impl std::fmt::Display for BoundedReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRegular => {
                write!(
                    f,
                    "not a regular file (a directory, FIFO, socket or device)"
                )
            }
            Self::TooLarge { max } => write!(f, "exceeds the {max} byte limit"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BoundedReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

/// Read at most `max_bytes` from `path`, which must be a regular file once any link on the way is
/// followed. For input that is repository or user content and must not be trusted to be a file.
///
/// The type is checked on the OPENED descriptor (`fstat`), not on the path, so nothing swapped in
/// after the open changes what is read; a directory, FIFO or character/block device is refused
/// without a read. A FIFO nobody writes to cannot hold the open: on unix it is
/// `O_NONBLOCK | O_NOCTTY` (the second so that opening a terminal device cannot make it this
/// process's controlling terminal). **The open itself still happens before the refusal**, so a
/// device is opened (never read) before `fstat` turns it away; its driver's open side effects,
/// if any, are not prevented here. A socket cannot be opened at all: `open(2)` fails `ENXIO`, which
/// is reported as the same [`BoundedReadError::NotRegular`]. The length is then bounded twice: the
/// reported length first (refuse without reading), and the read itself ([`read_bounded`]),
/// because a length is something the file reports about itself.
///
/// Links are followed on purpose: a `project.toml` that is a link to a real file is an accepted
/// layout. Pattern: `MemoryStore`'s `read_regular_file`, which refuses links instead.
pub fn read_regular_bytes(path: &Path, max_bytes: u64) -> Result<Vec<u8>, BoundedReadError> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        #[cfg(unix)]
        Err(e) if e.raw_os_error() == Some(libc::ENXIO) => {
            return Err(BoundedReadError::NotRegular);
        }
        Err(e) => return Err(BoundedReadError::Io(e)),
    };
    let metadata = file.metadata().map_err(BoundedReadError::Io)?;
    if !metadata.file_type().is_file() {
        return Err(BoundedReadError::NotRegular);
    }
    if metadata.len() > max_bytes {
        return Err(BoundedReadError::TooLarge { max: max_bytes });
    }
    read_bounded(file, max_bytes)
}

/// Read `reader` to its end, but never more than `max_bytes + 1` bytes from it: more than
/// `max_bytes` is [`BoundedReadError::TooLarge`]. The bound is on the bytes pulled from the
/// reader, so an endless or lying source cannot grow the buffer past `max_bytes + 1`.
fn read_bounded(reader: impl std::io::Read, max_bytes: u64) -> Result<Vec<u8>, BoundedReadError> {
    use std::io::Read;
    let mut bytes = Vec::new();
    reader
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(BoundedReadError::Io)?;
    if bytes.len() as u64 > max_bytes {
        return Err(BoundedReadError::TooLarge { max: max_bytes });
    }
    Ok(bytes)
}

/// [`read_regular_bytes`] as UTF-8 text; bytes that are not UTF-8 are an `Io` error of kind
/// `InvalidData`, as `std::fs::read_to_string` gives.
pub fn read_regular_text(path: &Path, max_bytes: u64) -> Result<String, BoundedReadError> {
    String::from_utf8(read_regular_bytes(path, max_bytes)?).map_err(|_| {
        BoundedReadError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "stream did not contain valid UTF-8",
        ))
    })
}

/// [`read_regular_bytes`] at [`MAX_CONFIG_FILE_BYTES`].
pub fn read_regular_config_bytes(path: &Path) -> Result<Vec<u8>, BoundedReadError> {
    read_regular_bytes(path, MAX_CONFIG_FILE_BYTES)
}

/// [`read_regular_text`] at [`MAX_CONFIG_FILE_BYTES`]: the one way a config loader reads a file.
pub fn read_regular_config_text(path: &Path) -> Result<String, BoundedReadError> {
    read_regular_text(path, MAX_CONFIG_FILE_BYTES)
}
/// The most bytes a git pointer file may hold: a worktree's `.git` file (`gitdir: <path>`), the
/// `gitdir` file under `.git/worktrees/<name>/`, or a `HEAD` (a SHA or `ref: refs/heads/<name>`).
/// Each is a line or two; the limit is far above any path or ref name and far below anything that
/// matters to memory.
// cap-class: NOT_A_CAP — bounds how many bytes of an INPUT git pointer file are read; a file over it is treated as absent, no result is shortened and nothing is presented as complete
pub const MAX_GIT_POINTER_BYTES: u64 = 64 * 1024;

/// Read a git pointer file (see [`MAX_GIT_POINTER_BYTES`]) as text, or `None` when it cannot be
/// used: absent, unreadable, not UTF-8, not a regular file, or over the limit. The path is a
/// `.git` file in a directory the server was pointed at, or a path that file names (so anyone who
/// wrote the pointer chose it), and a plain read of a FIFO there holds the caller for good. The
/// reason, never the content, is logged at debug.
pub fn read_git_pointer(path: &Path) -> Option<String> {
    match read_regular_text(path, MAX_GIT_POINTER_BYTES) {
        Ok(text) => Some(text),
        Err(e) => {
            if !e.is_not_found() {
                tracing::debug!("git pointer {} skipped: {e}", path.display());
            }
            None
        }
    }
}

/// How many staging names `atomic_write` tries before it gives up. A collision needs an
/// existing entry at a 128-bit random name, so reaching even the second attempt means
/// something is planting names; sixteen is generous for a benign cause and still bounded.
const STAGING_ATTEMPTS: u32 = 16;

/// How many bytes of the target's file name a staging name carries. Staging adds
/// `.<32 hex>.tmp` (37 bytes), and a name already near `NAME_MAX` (255) would otherwise
/// turn a write that used to fit into `ENAMETOOLONG`.
// cap-class: NOT_A_CAP — bounds only how much of the target's file name is echoed into the throwaway staging name; the staged content and the final file name are untouched, and uniqueness comes from the nonce
const STAGING_STEM_MAX_BYTES: usize = 100;

/// A staging path for `path`: `<file name prefix>.<nonce>.tmp` in the target's own directory.
/// `None` when `path` has no final component (a bare root, or one ending in `..`).
///
/// **Collision-freedom and safety come from the nonce plus the exclusive create in
/// [`create_staging_file`], not from the file-name prefix.** The prefix (the target's file name,
/// cut to `STAGING_STEM_MAX_BYTES` bytes) is kept so a leftover staging file names its target,
/// and because it was once the guarantee: `Path::with_extension("tmp")` derived the name from
/// the stem alone, so two concurrent writers to `Cargo.toml` and `Cargo.lock` renamed *each
/// other's* content onto their own targets (`7` same-stem groups were tracked in this repo on
/// 2026-08-31, `source.md`/`source.rs` among them;
/// `docs/issues/archive/2026-08-31-atomic-write-tmp-path-collides-across-same-stem-files.md`).
/// With a random nonce two writers cannot share a staging name whatever their file names are.
///
/// A fixed name was also a name anyone could plant in advance — a repository can ship
/// `<name>.tmp` as a symlink — and `fs::write` followed it. The nonce keeps the name
/// unguessable; the exclusive create makes a guessed or colliding one harmless.
///
/// The nonce is a parameter so a test can hold it constant and observe the prefix on its own:
/// with a random nonce every pair of names is distinct and that test could not fail.
///
/// The staging file stays a sibling of the target: the rename that finishes `atomic_write`
/// fails across filesystems, and sibling placement is what makes it atomic.
///
/// The sibling `with_extension` uses at `src/lsp/mux/mod.rs` and `src/retrieval/index_lock.rs`
/// are **not** instances of the stem defect: both take an internally-built `*.lock` path.
fn staging_path(path: &Path, nonce: &str) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy();
    let mut end = name.len().min(STAGING_STEM_MAX_BYTES);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    Some(path.with_file_name(format!("{}.{nonce}.tmp", &name[..end])))
}

/// An unpredictable nonce for [`staging_path`].
fn fresh_staging_nonce() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Create the staging file for `atomic_write`, **exclusively**: `create_new` is
/// `O_CREAT | O_EXCL` (`CREATE_NEW` on Windows), which fails if ANYTHING already exists at the
/// name — a regular file, a directory, a symlink, even a dangling one — and never follows a
/// link **at the final component**. So the file returned is one this call created, and an
/// entry planted at that name in advance can neither redirect the write nor be overwritten
/// by it.
///
/// **This says nothing about the directory part of the path.** A symlinked PARENT component
/// redirects both this create and the closing rename (measured), exactly as it redirects any
/// path-based write. For the write tools that is the write classifier's job
/// (`util::path_security`, which resolves the parent); a caller that bypasses it — for example
/// `MemoryStore::ensure_gitignored` (`memory/mod.rs`) writing `.gitignore` — gets no parent
/// protection from this function.
///
/// `private` creates the file `0600` (Unix; the umask can only narrow that) instead of the
/// default `0666 & !umask`. `atomic_write` uses it when the target already exists, so that a
/// private target's new content is not readable by other users while it is being written; the
/// target's own mode is applied afterwards.
///
/// On `AlreadyExists` the next name is tried, up to [`STAGING_ATTEMPTS`]; the colliding entry
/// is left exactly as found (it is not ours to remove). Any other error is returned at once.
fn create_staging_file(
    path: &Path,
    next_name: &mut dyn FnMut() -> Option<PathBuf>,
    private: bool,
) -> std::io::Result<(std::fs::File, PathBuf)> {
    for _ in 0..STAGING_ATTEMPTS {
        let Some(tmp) = next_name() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("{} has no file name to write to", path.display()),
            ));
        };
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        if private {
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        }
        #[cfg(not(unix))]
        let _ = private;
        match options.open(&tmp) {
            Ok(file) => return Ok((file, tmp)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!(
            "could not create an unused staging file beside {} after {STAGING_ATTEMPTS} \
             attempts: every candidate name already exists",
            path.display()
        ),
    ))
}

/// Atomic write: write to a uniquely named sibling staging file, then rename it onto `path`,
/// so a crash or disk-full condition mid-write can't leave the target in a corrupt state.
/// The target file must have a parent directory (true for all real paths).
///
/// **The staging file is created exclusively and never follows a link at its own name.** See
/// [`create_staging_file`]. Before this, staging was `fs::write(<name>.tmp)`, so a symlink
/// planted at `<name>.tmp` — shipped in a cloned repository, say — redirected the write to
/// whatever it pointed at and the rename then moved the link onto the target. An existing
/// entry at a staging name is now skipped over, never written through, replaced or removed.
/// `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`.
///
/// **A target that is itself a symlink is replaced, not written through.** `rename` swaps the
/// directory entry, so the file the link pointed at keeps its content and `path` becomes a
/// regular file. (The mode copied below is read with `metadata`, which follows the link, so
/// the new regular file takes the pointed-at file's permission bits; for a dangling link
/// there is nothing to read and the default applies.)
///
/// **Leftovers.** A process killed between the create and the rename leaves a uniquely named
/// `<name prefix>.<32 hex>.tmp` beside the target, and nothing reclaims it. A stale legacy
/// `<name>.tmp` from before this change is never touched either (a fixed name is no longer
/// used, so it is neither reused nor removed). `.tmp` is not in this repository's `.gitignore`,
/// so a leftover shows in `git status`; it is not embedded (`retrieval/sync.rs`) and not
/// catalogued (`librarian/indexer.rs` takes `.md` only). A reaper is possible and not built.
///
/// Preserves the target file's Unix permissions (e.g. exec bit) across the
/// rename. Without this, editing a `*.sh` script would silently strip +x
/// because the freshly-created staging file has default 0644 perms. For an existing target the
/// staging file is created `0600` and given the target's mode after the write, so the new
/// content of a private file is not readable by other users during the write; a new file keeps
/// the `0666 & !umask` default.
///
/// **The final mode is unchanged in every case where `fchmod` succeeds.** A failing `fchmod`
/// is ignored, as it always was, and for an existing target that now leaves the file at
/// `0600 & !umask` instead of `0666 & !umask` — it fails closed, a `0644`/`0755` file narrows
/// to `0600`, and the write still succeeds. Plausible hosts: an sshfs mount whose sftp-server
/// refuses `setstat`/`fsetstat`, an LSM (SELinux) denying `setattr`, sticky/setgid rules on
/// macOS/FreeBSD. Ignoring is deliberate (the content is written, and refusing the whole edit
/// over a mode would be worse); it is documented, not changed.
///
/// **The mode is read BEFORE the create, which widens the race the next paragraph already
/// admits.** Measured by the reviewer against the old read-after-write order: a target created
/// `0600` by someone else during our write ends `0644` (the default) where it used to end
/// `0600`; a `chmod 0644 -> 0755` during the write is lost where it used to be picked up (old
/// `0755`); a target deleted during the write keeps `0600` where it used to end `0644`.
///
/// Not fsynced, as before: atomic against a concurrent reader and a failed write, not
/// durable across power loss. Two writers to the *same* target still race (last rename wins).
pub fn atomic_write(path: &Path, content: &str) -> std::io::Result<()> {
    // Before anything is created: a refused write leaves no staging file behind. This is the
    // funnel `create_file`, `edit_file` and `edit_code` all write through, which is why the
    // guard on codescout's own config files lives here and not in each tool.
    crate::config::protected_files::check_write(path, content)
        .map_err(|msg| std::io::Error::new(std::io::ErrorKind::PermissionDenied, msg))?;
    atomic_write_with(
        path,
        content,
        &mut || staging_path(path, &fresh_staging_nonce()),
        &mut |file, bytes| std::io::Write::write_all(file, bytes),
    )
}

/// The body of [`atomic_write`], with the two things a test cannot otherwise steer as
/// parameters: the sequence of staging names offered, and the write into the staging file.
///
/// The names let a test make the FIRST candidate a planted link, so the exclusive create is
/// what refuses it — with random names a planted `x.txt.tmp` is simply never opened and a
/// test of it passes against any implementation. The write lets a test fail it after the
/// create succeeded (ENOSPC), which the old test did by planting `<name>.tmp -> /dev/full`:
/// that planting IS the vulnerability, so it can no longer be the way in. It also runs after
/// the create and before the final mode is set, which is where the private-target window can
/// be observed.
fn atomic_write_with(
    path: &Path,
    content: &str,
    next_name: &mut dyn FnMut() -> Option<PathBuf>,
    write: &mut dyn FnMut(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
) -> std::io::Result<()> {
    // The mode to preserve, if the target already exists (`metadata` follows a symlink target,
    // so a replaced link takes the mode of what it pointed at). Read BEFORE the create, because
    // an existing target is also what makes the staging file private until it is applied.
    #[cfg(unix)]
    let existing_mode = std::fs::metadata(path).ok().map(|m| {
        use std::os::unix::fs::PermissionsExt;
        m.permissions().mode()
    });
    #[cfg(not(unix))]
    let existing_mode: Option<u32> = None;

    let (mut file, tmp) = create_staging_file(path, next_name, existing_mode.is_some())?;

    // From here `tmp` is a file THIS call created, so removing it is always ours to do.
    // Cleanup is needed on BOTH failure points, not just the rename below: under ENOSPC
    // the create can SUCCEED — the staging file now exists — while the write fails.
    // The target itself is always safe, because only the rename touches it.
    if let Err(e) = write(&mut file, content.as_bytes()) {
        drop(file);
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }

    // Preserve the original mode. Through the open handle (`fchmod`), not the path: no window
    // in which the path could be swapped.
    #[cfg(unix)]
    if let Some(mode) = existing_mode {
        use std::os::unix::fs::PermissionsExt;
        let _ = file.set_permissions(std::fs::Permissions::from_mode(mode));
    }

    // Closed before the rename: Windows will not rename an open file.
    drop(file);
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

/// `path` without Windows' `\\?\` verbatim marker, so a canonicalized path compares
/// equal to — and `starts_with` — the same location spelled without it.
///
/// Only Windows has the two spellings. `fs::canonicalize` there returns `\\?\C:\…`,
/// while a `file://` URI cannot carry the marker, so every path
/// `FileAddress::from_lsp_uri` yields is `C:\…`. `Path::starts_with` compares
/// components, and the prefixes parse as `VerbatimDisk('C')` against `Disk('C')`, so
/// no location a language server reports is ever under a canonical project root
/// (`fed1c5731a623c30`). Verbatim disk and UNC prefixes are rewritten; every other
/// path, and every path on other platforms, is returned borrowed and unchanged.
///
/// A `Path` operation rather than [`to_forward_slash`]-style string surgery, because
/// callers need component-aware containment, not string equality.
pub fn strip_verbatim(path: &std::path::Path) -> std::borrow::Cow<'_, std::path::Path> {
    use std::borrow::Cow;
    use std::path::{Component, PathBuf, Prefix};

    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Cow::Borrowed(path);
    };
    let plain = match prefix.kind() {
        Prefix::VerbatimDisk(drive) => format!("{}:", drive as char),
        Prefix::VerbatimUNC(server, share) => format!(
            r"\\{}\{}",
            server.to_string_lossy(),
            share.to_string_lossy()
        ),
        _ => return Cow::Borrowed(path),
    };
    let mut out = PathBuf::from(plain);
    for component in components {
        out.push(component.as_os_str());
    }
    Cow::Owned(out)
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

/// Fixtures for tests that put a special file where a config file belongs. Unix only: a FIFO and
/// a link need it. A read of a FIFO nobody writes to never returns, so a test that calls a loader
/// on one goes through [`within_deadline`], which turns the hang into a failing assertion.
#[cfg(all(test, unix))]
pub(crate) mod special_file_fixtures {
    use std::path::Path;
    use std::time::Duration;

    /// How long a loader may take on a special file before the test calls it hung. A refusal is
    /// one `open` and one `fstat`; the slack is for a loaded machine.
    pub(crate) const HANG_DEADLINE: Duration = Duration::from_secs(15);

    /// `mkfifo(path)`: a named pipe with no writer.
    pub(crate) fn make_fifo(path: &Path) {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: `c` is a valid NUL-terminated path for the duration of the call.
        let rc = unsafe { libc::mkfifo(c.as_ptr(), 0o600) };
        assert_eq!(rc, 0, "mkfifo failed: {}", std::io::Error::last_os_error());
    }

    /// Run `f` on a thread and return its result, or panic if it has not finished within
    /// [`HANG_DEADLINE`]. A thread stuck in a blocking `open` is left behind (it cannot be
    /// cancelled); the test binary exits when the suite is done.
    #[track_caller]
    pub(crate) fn within_deadline<T: Send + 'static>(
        what: &str,
        f: impl FnOnce() -> T + Send + 'static,
    ) -> T {
        within_deadline_of(what, HANG_DEADLINE, f)
    }

    /// [`within_deadline`] with the caller's own deadline, for a call that is slow on its own.
    #[track_caller]
    pub(crate) fn within_deadline_of<T: Send + 'static>(
        what: &str,
        deadline: Duration,
        f: impl FnOnce() -> T + Send + 'static,
    ) -> T {
        use std::sync::mpsc::RecvTimeoutError;
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        match rx.recv_timeout(deadline) {
            Ok(value) => value,
            Err(RecvTimeoutError::Timeout) => panic!(
                "{what} did not return within {deadline:?}: it is blocked reading a special file"
            ),
            // The sender is gone without a value: the closure panicked. Its own panic is the
            // failure, not a hang; re-raise it with its message.
            Err(RecvTimeoutError::Disconnected) => match handle.join() {
                Err(payload) => std::panic::resume_unwind(payload),
                Ok(()) => panic!("{what}: the worker ended without a result"),
            },
        }
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
    fn strip_verbatim_leaves_a_path_without_the_marker_borrowed() {
        // Every non-Windows path takes this branch, and so does a plain Windows one.
        let p = std::path::Path::new("/home/dev/proj/src/lib.rs");
        assert!(matches!(strip_verbatim(p), std::borrow::Cow::Borrowed(q) if q == p));
    }

    // Windows-only: no other platform parses a path prefix, so this cannot run anywhere
    // else. It runs on the windows-latest lanes and on the wine lane.
    #[cfg(windows)]
    #[test]
    fn strip_verbatim_makes_a_canonical_root_contain_its_uri_spelling() {
        use std::path::Path;
        let canonical_root = Path::new(r"\\?\C:\Users\dev\proj");
        let from_uri = Path::new(r"C:\Users\dev\proj\src\lib.rs");
        // Precondition: the raw spellings really do not match, or this proves nothing.
        assert!(!from_uri.starts_with(canonical_root));
        assert!(strip_verbatim(from_uri).starts_with(strip_verbatim(canonical_root)));
        assert_eq!(
            strip_verbatim(canonical_root),
            Path::new(r"C:\Users\dev\proj")
        );
        assert_eq!(
            strip_verbatim(Path::new(r"\\?\UNC\srv\share\proj")),
            Path::new(r"\\srv\share\proj")
        );
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

    /// The staging path's file-name prefix must differ per TARGET FILENAME, not per stem — held
    /// at a CONSTANT nonce so only the prefix is under test.
    ///
    /// Since the nonce, the prefix is no longer what makes two writers' staging files distinct
    /// (a random nonce does, and the exclusive create backstops it); this pins the remaining
    /// property that a leftover staging file names its own target, and that nobody re-derives
    /// the name from the stem. The history below is why that derivation is distrusted.
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
                // One nonce for every name: with a random one all names are distinct and
                // this test could not fail; held constant, only the filename half is left.
                let staged = staging_path(&dir.join(name), "nonce").unwrap();
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
        let staged = staging_path(target, "nonce").unwrap();

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
    // ---- The staging file must never be a link someone else planted ----------------------
    //
    // `docs/issues/2026-10-17-atomic-write-follows-a-planted-staging-symlink-and-overwrites-a-file-outside-the-root.md`.
    // `atomic_write` used to stage at `<name>.tmp` with `std::fs::write`, which follows a
    // symlink; the rename then moved the link onto the target. A repository can ship such a
    // link (git carries symlinks), so any codescout write to `x.txt` overwrote a file outside
    // the root. Every test below plants through the LEGACY staging name, so that against the
    // old derivation the planted entry is exactly the one `atomic_write` opens.

    /// `root/` (the "project"), `outside/victim.txt` holding `ORIGINAL`, in one tempdir.
    #[cfg(unix)]
    fn staging_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempdir().unwrap();
        let root = dir.path().join("root");
        let outside = dir.path().join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let victim = outside.join("victim.txt");
        std::fs::write(&victim, "ORIGINAL").unwrap();
        (dir, root, victim)
    }

    #[cfg(unix)]
    #[test]
    fn a_planted_staging_symlink_is_not_followed_and_the_victim_is_untouched() {
        let (_dir, root, victim) = staging_fixture();
        let planted = root.join("x.txt.tmp");
        std::os::unix::fs::symlink(&victim, &planted).unwrap();

        atomic_write(&root.join("x.txt"), "WRITTEN").unwrap();

        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "ORIGINAL",
            "atomic_write wrote through a planted staging symlink to a file outside the root"
        );
        let target_meta = std::fs::symlink_metadata(root.join("x.txt")).unwrap();
        assert!(
            target_meta.file_type().is_file(),
            "the target must be a regular file, not a link to the victim"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("x.txt")).unwrap(),
            "WRITTEN"
        );
        // A file we did not create is neither followed, nor moved, nor removed.
        assert_eq!(std::fs::read_link(&planted).unwrap(), victim);
    }

    #[cfg(unix)]
    #[test]
    fn a_planted_dangling_staging_symlink_does_not_create_the_file_it_points_at() {
        let (dir, root, _victim) = staging_fixture();
        let created = dir.path().join("outside").join("created-by-the-write.txt");
        let planted = root.join("x.txt.tmp");
        std::os::unix::fs::symlink(&created, &planted).unwrap();

        atomic_write(&root.join("x.txt"), "WRITTEN").unwrap();

        assert!(
            created.symlink_metadata().is_err(),
            "a dangling staging symlink made atomic_write create {created:?} outside the root"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("x.txt")).unwrap(),
            "WRITTEN"
        );
        assert!(planted.symlink_metadata().unwrap().file_type().is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn a_planted_regular_staging_file_is_not_clobbered() {
        let (_dir, root, _victim) = staging_fixture();
        let planted = root.join("x.txt.tmp");
        std::fs::write(&planted, "PLANTED").unwrap();

        atomic_write(&root.join("x.txt"), "WRITTEN").unwrap();

        assert_eq!(
            std::fs::read_to_string(&planted).unwrap(),
            "PLANTED",
            "atomic_write overwrote a file it did not create"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("x.txt")).unwrap(),
            "WRITTEN"
        );
    }

    /// The ordinary paths, so the fix cannot be "refuse everything". Covers a new file and an
    /// overwrite, and pins that neither leaves a staging file in the directory.
    #[cfg(unix)]
    #[test]
    fn a_new_file_and_an_overwrite_both_work_and_leave_no_staging_file() {
        let (_dir, root, _victim) = staging_fixture();
        let target = root.join("x.txt");

        atomic_write(&target, "first").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "first");
        assert_eq!(entry_names_any(&root), vec!["x.txt".to_string()]);

        atomic_write(&target, "second, longer").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "second, longer");
        assert_eq!(entry_names_any(&root), vec!["x.txt".to_string()]);
    }

    /// DECISION, for a target that is itself a symlink: `atomic_write` never writes THROUGH the
    /// leaf link. The rename replaces the link with a regular file, so the file the link
    /// pointed at keeps its content. That is what `edit_file` was already measured doing in
    /// `2026-10-17-write-allowlist-canonicalizes-only-the-parent...`; this pins it for the
    /// shared writer so a future "preserve the link" change cannot reopen an escape.
    #[cfg(unix)]
    #[test]
    fn a_target_that_is_a_symlink_is_replaced_not_written_through() {
        let (dir, root, victim) = staging_fixture();
        let leaf = root.join("leaf");
        std::os::unix::fs::symlink(&victim, &leaf).unwrap();

        atomic_write(&leaf, "WRITTEN").unwrap();

        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "ORIGINAL");
        assert!(std::fs::symlink_metadata(&leaf)
            .unwrap()
            .file_type()
            .is_file());
        assert_eq!(std::fs::read_to_string(&leaf).unwrap(), "WRITTEN");

        // Dangling leaf link: the write must not create the file it points at.
        let never = dir.path().join("outside").join("never-created.txt");
        let dangling = root.join("dangling");
        std::os::unix::fs::symlink(&never, &dangling).unwrap();
        atomic_write(&dangling, "WRITTEN").unwrap();
        assert!(never.symlink_metadata().is_err());
        assert!(std::fs::symlink_metadata(&dangling)
            .unwrap()
            .file_type()
            .is_file());
    }

    /// What `File::create` gives in this environment (`0666 & !umask`): the mode a new file
    /// gets today. Also the precondition for the window test below — under a umask that
    /// already hides group/other bits there is nothing to observe, and a test that cannot
    /// fail must say so rather than pass.
    #[cfg(unix)]
    fn default_new_file_mode(dir: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        let control = dir.join("umask-control");
        std::fs::File::create(&control).unwrap();
        let mode = std::fs::metadata(&control).unwrap().permissions().mode() & 0o7777;
        std::fs::remove_file(&control).unwrap();
        mode
    }

    /// While the content is being written the staging file holds the NEW content of a target
    /// that may be private. It was created `0666 & !umask` (0644), so another local user could
    /// read a `0600` target's new bytes for the length of the write. For an EXISTING target the
    /// staging file is now created `0600` and given the target's mode only after the write.
    ///
    /// Observed from inside the write step, which runs after the create and before the final
    /// mode is set, so this is the window itself rather than a before/after comparison.
    ///
    /// **Needs a umask that leaves group/other read open, and must not fail a host for lacking
    /// one.** Under `umask 077` (or 066/067/076) a plain create is already private, so the
    /// window cannot be seen. Rather than fail there, or pass vacuously, the test re-runs ITSELF
    /// in a child under `umask 022` (`sh -c 'umask 022; exec ...'`). The child is marked by an
    /// environment variable and asserts its own precondition, so a child that could not
    /// establish `022` fails loudly instead of passing.
    #[cfg(unix)]
    #[test]
    fn the_staging_file_of_a_private_target_is_private_while_it_is_written() {
        const MARK: &str = "CODESCOUT_FS_WINDOW_TEST_UMASK022_CHILD";
        let dir = tempdir().unwrap();
        let can_observe = default_new_file_mode(dir.path()) & 0o044 == 0o044;
        if !can_observe && std::env::var_os(MARK).is_none() {
            let exe = std::env::current_exe().unwrap();
            let out = std::process::Command::new("sh")
                .arg("-c")
                .arg(r#"umask 022; exec "$0" "$@""#)
                .arg(exe)
                .args([
                    "--exact",
                    "util::fs::tests::the_staging_file_of_a_private_target_is_private_while_it_is_written",
                    "--test-threads=1",
                ])
                .env(MARK, "1")
                .output()
                .expect("re-run under umask 022");
            assert!(
                out.status.success(),
                "the umask-022 child failed:\n{}\n{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            // The child must have RUN the body, not filtered it out.
            assert!(
                String::from_utf8_lossy(&out.stdout).contains("1 passed"),
                "the umask-022 child ran no test:\n{}",
                String::from_utf8_lossy(&out.stdout)
            );
            return;
        }
        private_window_body(dir.path());
    }

    /// The body of the window test; asserts its own precondition, so it is loud wherever it runs.
    #[cfg(unix)]
    fn private_window_body(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            default_new_file_mode(dir) & 0o044,
            0o044,
            "this process's umask hides group/other read, so the window cannot be observed — \
             the umask-022 re-run did not establish 022"
        );
        let target = dir.join("secret.txt");
        std::fs::write(&target, "old").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600)).unwrap();

        let mut seen_during_write = None;
        atomic_write_with(
            &target,
            "new secret",
            &mut || staging_path(&target, &fresh_staging_nonce()),
            &mut |f, bytes| {
                seen_during_write = Some(f.metadata()?.permissions().mode() & 0o777);
                std::io::Write::write_all(f, bytes)
            },
        )
        .unwrap();

        assert_eq!(
            seen_during_write.expect("the write step must run") & 0o077,
            0,
            "the staging file was readable by group/other while it held the new content"
        );
        assert_eq!(
            std::fs::metadata(&target).unwrap().permissions().mode() & 0o7777,
            0o600
        );
    }

    /// The hardening above must not move the FINAL mode in any case. Each row is the mode
    /// `atomic_write` gave before it: an existing target keeps its own (setuid included), a new
    /// file gets the umask default, and a symlink target gets the mode of what it points at
    /// (`metadata` follows the link; `symlink_metadata` would give 0777) while a dangling one
    /// gets the default.
    #[cfg(unix)]
    #[test]
    fn the_final_mode_is_unchanged_for_every_kind_of_target() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let default = default_new_file_mode(dir.path());
        let mode_of = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o7777;
        let chmod = |p: &Path, m: u32| {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(m)).unwrap()
        };

        for existing in [0o600, 0o640, 0o755, 0o4755] {
            let t = dir.path().join(format!("existing-{existing:o}"));
            std::fs::write(&t, "old").unwrap();
            chmod(&t, existing);
            atomic_write(&t, "new").unwrap();
            assert_eq!(mode_of(&t), existing, "existing target {existing:o}");
            assert_eq!(std::fs::read_to_string(&t).unwrap(), "new");
        }

        let fresh = dir.path().join("fresh");
        atomic_write(&fresh, "new").unwrap();
        assert_eq!(
            mode_of(&fresh),
            default,
            "a new file keeps the umask default"
        );

        let pointed_at = dir.path().join("pointed-at");
        std::fs::write(&pointed_at, "old").unwrap();
        chmod(&pointed_at, 0o640);
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&pointed_at, &link).unwrap();
        atomic_write(&link, "new").unwrap();
        assert_eq!(
            mode_of(&link),
            0o640,
            "a replaced symlink takes the mode of what it pointed at, never the link's 0777"
        );
        assert_eq!(std::fs::read_to_string(&pointed_at).unwrap(), "old");

        let dangling = dir.path().join("dangling");
        std::os::unix::fs::symlink(dir.path().join("nowhere"), &dangling).unwrap();
        atomic_write(&dangling, "new").unwrap();
        assert_eq!(mode_of(&dangling), default);
    }

    /// The real staging names must stay on the target's filesystem, or the closing `rename`
    /// fails with EXDEV. Seam-free: a target on one device and `std::env::temp_dir()` on
    /// another means a staging file placed in the temp dir makes the write fail.
    ///
    /// This catches staging on a DIFFERENT DEVICE and nothing more: a closure that stages in
    /// the grandparent on the same filesystem, or reuses a nonce, survives it. Those are the
    /// jobs of `the_real_staging_file_is_beside_its_target_not_in_its_parent` and
    /// `concurrent_writers_to_one_target_never_collide_on_a_staging_name`.
    ///
    /// Linux only (`/dev/shm`, `/var/tmp`). A host where neither is on a different device than
    /// `temp_dir()` (a container without its own `/dev/shm`, say) cannot run it: it then SKIPS,
    /// printing each candidate's finding and re-checking that finding independently, instead of
    /// failing a legitimate host. Where a distinct device exists it asserts as before.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_real_staging_file_is_created_on_the_targets_filesystem() {
        use std::os::unix::fs::MetadataExt;
        let temp_dev = std::fs::metadata(std::env::temp_dir()).unwrap().dev();
        let mut findings = Vec::new();
        let mut other = None;
        for c in ["/dev/shm", "/var/tmp"] {
            match tempfile::tempdir_in(c) {
                Err(e) => findings.push(format!("{c}: not writable ({e})")),
                Ok(d) => {
                    let dev = std::fs::metadata(d.path()).unwrap().dev();
                    if dev == temp_dev {
                        findings.push(format!("{c}: same device as temp_dir() ({dev})"));
                    } else {
                        other = Some(d);
                        break;
                    }
                }
            }
        }
        let Some(other) = other else {
            // The skip reason is asserted, not assumed: every candidate must have been rejected
            // for one of the two recorded reasons, and re-checking must agree.
            assert_eq!(findings.len(), 2, "every candidate needs a recorded reason");
            for c in ["/dev/shm", "/var/tmp"] {
                let distinct = tempfile::tempdir_in(c)
                    .ok()
                    .map(|d| std::fs::metadata(d.path()).unwrap().dev() != temp_dev)
                    .unwrap_or(false);
                assert!(!distinct, "{c} is usable after all; the skip is wrong");
            }
            eprintln!(
                "SKIPPED the_real_staging_file_is_created_on_the_targets_filesystem: no writable \
                 directory on a different device than temp_dir(): {findings:?}"
            );
            return;
        };
        let target = other.path().join("x.txt");

        atomic_write(&target, "WRITTEN").unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "WRITTEN");
        assert_eq!(entry_names_any(other.path()), vec!["x.txt".to_string()]);
    }

    /// Restores a directory's permissions on drop, so a failed assertion cannot leave a
    /// read-only directory behind for the tempdir cleanup to trip over.
    #[cfg(unix)]
    struct RestoreMode(PathBuf);
    #[cfg(unix)]
    impl Drop for RestoreMode {
        fn drop(&mut self) {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
        }
    }

    /// The staging file must be created in the target's OWN directory, not in an ancestor.
    /// Seam-free, through the real name closure: the target's directory is writable but its
    /// PARENT is read-only, so a closure that staged one level up (same filesystem, which the
    /// cross-device test cannot see) would fail the create with EACCES. A privileged user
    /// ignores directory modes, so as root the test SKIPS, saying so.
    #[cfg(unix)]
    #[test]
    fn the_real_staging_file_is_beside_its_target_not_in_its_parent() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            eprintln!(
                "SKIPPED the_real_staging_file_is_beside_its_target_not_in_its_parent: running as \
                 root, directory modes do not restrict it"
            );
            return;
        }
        let dir = tempdir().unwrap();
        let parent = dir.path().join("parent");
        let own = parent.join("own");
        std::fs::create_dir_all(&own).unwrap();
        let _restore = RestoreMode(parent.clone());
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Precondition: the restriction really bites, or this proves nothing.
        assert!(
            std::fs::File::create(parent.join("probe")).is_err(),
            "the read-only parent still accepts a create; the test cannot discriminate"
        );

        let target = own.join("x.txt");
        atomic_write(&target, "WRITTEN").unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "WRITTEN");
        assert_eq!(entry_names_any(&own), vec!["x.txt".to_string()]);
    }

    /// The test the three above cannot be: with random staging names a planted `x.txt.tmp` is
    /// never even opened, so they would pass against ANY implementation that merely renamed
    /// its staging file. Here the planted entries are the FIRST candidates offered, so it is
    /// the exclusive create that has to refuse each one — a symlink to a victim, a dangling
    /// symlink, a regular file, a directory — and move on to the next name.
    #[cfg(unix)]
    #[test]
    fn every_kind_of_planted_entry_at_a_candidate_name_is_refused_and_left_alone() {
        let (dir, root, victim) = staging_fixture();
        let never = dir.path().join("outside").join("never-created.txt");

        let p_link = root.join("c1.tmp");
        let p_dangling = root.join("c2.tmp");
        let p_regular = root.join("c3.tmp");
        let p_dir = root.join("c4.tmp");
        let fresh = root.join("c5.tmp");
        std::os::unix::fs::symlink(&victim, &p_link).unwrap();
        std::os::unix::fs::symlink(&never, &p_dangling).unwrap();
        std::fs::write(&p_regular, "PLANTED").unwrap();
        std::fs::create_dir(&p_dir).unwrap();

        let mut queue: std::collections::VecDeque<PathBuf> =
            [&p_link, &p_dangling, &p_regular, &p_dir, &fresh]
                .into_iter()
                .cloned()
                .collect();
        let target = root.join("x.txt");
        atomic_write_with(
            &target,
            "WRITTEN",
            &mut || queue.pop_front(),
            &mut |f, b| std::io::Write::write_all(f, b),
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "ORIGINAL");
        assert!(
            never.symlink_metadata().is_err(),
            "dangling link was followed"
        );
        assert_eq!(std::fs::read_link(&p_link).unwrap(), victim);
        assert_eq!(std::fs::read_link(&p_dangling).unwrap(), never);
        assert_eq!(std::fs::read_to_string(&p_regular).unwrap(), "PLANTED");
        assert!(p_dir.is_dir());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "WRITTEN");
        // The fresh candidate was consumed by the rename, and nothing else was added.
        assert!(fresh.symlink_metadata().is_err());
        assert_eq!(
            entry_names_any(&root),
            vec!["c1.tmp", "c2.tmp", "c3.tmp", "c4.tmp", "x.txt"]
        );
    }

    /// Collisions are retried a BOUNDED number of times, then fail with a clear error. Offered
    /// the same planted name forever, `atomic_write_with` must stop after exactly
    /// `STAGING_ATTEMPTS` tries, create and change nothing, and say why.
    #[cfg(unix)]
    #[test]
    fn staging_gives_up_after_a_bounded_number_of_collisions_and_changes_nothing() {
        let (_dir, root, victim) = staging_fixture();
        let planted = root.join("always.tmp");
        std::os::unix::fs::symlink(&victim, &planted).unwrap();
        let target = root.join("x.txt");

        let mut offered = 0u32;
        let err = atomic_write_with(
            &target,
            "WRITTEN",
            &mut || {
                offered += 1;
                Some(planted.clone())
            },
            &mut |f, b| std::io::Write::write_all(f, b),
        )
        .expect_err("a staging name that always collides must be an error");

        assert_eq!(
            offered, STAGING_ATTEMPTS,
            "the retry loop must be bounded and use its budget"
        );
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(
            err.to_string().contains("staging"),
            "the error must say what ran out: {err}"
        );
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "ORIGINAL");
        assert_eq!(std::fs::read_link(&planted).unwrap(), victim);
        assert!(
            target.symlink_metadata().is_err(),
            "the target must not appear"
        );
        assert_eq!(entry_names_any(&root), vec!["always.tmp"]);
    }

    /// Staging names are unpredictable, so planting one in advance is a guess; and they stay
    /// beside the target. A fixed derivation (the old `<name>.tmp`) fails the first two asserts.
    #[test]
    fn real_staging_names_are_unpredictable_and_not_the_legacy_name() {
        let target = Path::new("/repo/src/x.txt");
        let a = staging_path(target, &fresh_staging_nonce()).unwrap();
        let b = staging_path(target, &fresh_staging_nonce()).unwrap();
        assert_ne!(a, b, "two staging names for one target must differ");
        assert_ne!(
            a,
            Path::new("/repo/src/x.txt.tmp"),
            "the legacy name is plantable"
        );
        assert_eq!(a.parent(), target.parent());
        assert!(a.to_string_lossy().ends_with(".tmp"));
    }

    /// A file name near `NAME_MAX` (255 bytes) must still be writable: the staging name adds a
    /// nonce, and unbounded growth would turn it into ENAMETOOLONG. The multi-byte name makes
    /// the truncation land inside a character, which must back off to a boundary, not panic.
    #[cfg(unix)]
    #[test]
    fn a_file_name_near_the_name_max_still_writes() {
        let dir = tempdir().unwrap();
        for name in ["a".repeat(250), "あ".repeat(82)] {
            assert!(name.len() >= 246 && name.len() <= 255, "{}", name.len());
            let target = dir.path().join(&name);
            atomic_write(&target, "ok").unwrap();
            assert_eq!(std::fs::read_to_string(&target).unwrap(), "ok");
        }
        assert_eq!(
            entry_names_any(dir.path()).len(),
            2,
            "a staging file leaked"
        );
    }

    #[test]
    fn a_missing_parent_directory_is_an_error_not_a_retry_loop() {
        let dir = tempdir().unwrap();
        let err = atomic_write(&dir.path().join("no-such-dir").join("x.txt"), "x").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    /// The rename is the other failure point after a successful create. A target that is a
    /// directory makes `rename` fail; the staging file the call created must not outlive it.
    #[test]
    fn a_failed_rename_leaves_no_staging_file_and_the_target_untouched() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("sub");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("inner.txt"), "inner").unwrap();

        let err = atomic_write(&target, "x").expect_err("renaming a file over a directory fails");

        assert!(
            err.raw_os_error().is_some(),
            "expected the OS's refusal, got {err:?}"
        );
        assert_eq!(
            entry_names_any(dir.path()),
            vec!["sub".to_string()],
            "the staging file leaked after the rename failed"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("inner.txt")).unwrap(),
            "inner"
        );
    }

    #[test]
    fn a_path_with_no_file_name_is_refused_up_front() {
        let dir = tempdir().unwrap();
        let dotdot = dir.path().join("..");
        assert!(
            dotdot.file_name().is_none(),
            "precondition: `..` has no file name"
        );
        for p in [dotdot.as_path(), Path::new("/")] {
            let err = atomic_write(p, "x").unwrap_err();
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "{p:?}");
        }
    }

    /// Same-stem files written concurrently — the archived stem-collision scenario, now with
    /// unique staging names too. Every file must end up holding one of ITS OWN writes, and no
    /// staging file may survive.
    // Unix only: Windows may refuse a rename onto a file another thread has open.
    #[cfg(unix)]
    #[test]
    fn concurrent_writers_to_same_stem_files_never_cross_or_leak() {
        let dir = tempdir().unwrap();
        let names = ["Cargo.toml", "Cargo.lock", "source.md", "source.rs"];
        std::thread::scope(|s| {
            for name in names {
                // Two threads per file: same-target writers race on the rename, not the staging.
                for writer in 0..2 {
                    let path = dir.path().join(name);
                    s.spawn(move || {
                        for i in 0..40 {
                            atomic_write(&path, &format!("{name}:{writer}:{i}")).unwrap();
                        }
                    });
                }
            }
        });
        for name in names {
            let got = std::fs::read_to_string(dir.path().join(name)).unwrap();
            assert!(got.starts_with(&format!("{name}:")), "{name} holds {got:?}");
        }
        let mut want: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        want.sort();
        assert_eq!(entry_names_any(dir.path()), want, "a staging file leaked");
    }

    /// Many writers to ONE target through the real name closure. Every call must succeed: a
    /// closure that offered the same staging name to every call (a constant nonce) would make
    /// concurrent creates collide, burn their 16 attempts on the one occupied name and fail.
    /// The payload is large so each writer holds its staging file long enough for the others
    /// to meet it; a correct implementation never fails this, whatever the timing.
    #[cfg(unix)]
    #[test]
    fn concurrent_writers_to_one_target_never_collide_on_a_staging_name() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("shared.txt");
        let payload = "x".repeat(512 * 1024);
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    for _ in 0..25 {
                        atomic_write(&target, &payload)
                            .expect("a concurrent write must not collide");
                    }
                });
            }
        });
        assert_eq!(std::fs::read_to_string(&target).unwrap(), payload);
        assert_eq!(
            entry_names_any(dir.path()),
            vec!["shared.txt".to_string()],
            "a staging file leaked"
        );
    }

    /// The leak the `rename` guard never covered. `std::fs::write` is
    /// `File::create` + `write_all`, so under ENOSPC the create SUCCEEDS — the staging
    /// file exists from that moment — and the write then fails. The `?` propagated
    /// straight out, leaving the sibling `.tmp` behind.
    ///
    /// **Reproducing it requires create to succeed and the write to fail**, and
    /// that is the whole difficulty. The obvious reproductions — making the staging
    /// path a directory, or its parent read-only — fail at the create instead,
    /// so nothing is created, nothing leaks, and such a test PASSES against the
    /// unfixed function. `/dev/full` gives the right shape: the open succeeds and
    /// every write returns ENOSPC.
    ///
    /// **How `/dev/full` gets in changed with the staging fix.** This test used to plant
    /// `target.tmp -> /dev/full` so that `atomic_write` itself opened it — which IS the
    /// vulnerability (a planted staging link followed), and is now refused. So the real create
    /// still happens, exclusively, and the injected write sends the bytes to `/dev/full`
    /// instead: the kernel still returns the real ENOSPC, after a real create.
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

        // Pre-existing content, so the atomicity guarantee is pinned as "unchanged"
        // rather than merely "not created" — the stronger of the two properties, and
        // the one the .tmp-then-rename dance exists for.
        const ORIGINAL: &str = "original content, must survive a failed write\n";
        std::fs::write(&target, ORIGINAL).unwrap();

        // Positive control for the create: the writer sees the staging file already
        // on disk, so a leak below is a leak of a file that really existed.
        let mut staging_seen_by_writer: Option<PathBuf> = None;
        let mut names = {
            let target = target.clone();
            move || staging_path(&target, &fresh_staging_nonce())
        };
        let dir_path = dir.path().to_path_buf();
        let err = atomic_write_with(&target, "new content", &mut names, &mut |_staged, bytes| {
            let on_disk = entry_names_any(&dir_path);
            staging_seen_by_writer = on_disk
                .into_iter()
                .find(|n| n.ends_with(".tmp"))
                .map(PathBuf::from);
            // The real ENOSPC, from the kernel.
            let mut full = std::fs::OpenOptions::new().write(true).open("/dev/full")?;
            std::io::Write::write_all(&mut full, bytes)
        })
        // (1) It must FAIL. A version that swallowed the error and returned Ok would
        // satisfy the cleanup assertion below while breaking the function's purpose.
        .expect_err("atomic_write must propagate the write failure, not swallow it");

        // (2) Positive control: pin that we reproduced the INTENDED failure. Without
        // it, a mechanism that silently started yielding some other error (EACCES,
        // say) would still satisfy (3) and (4) while exercising a different path.
        assert_eq!(
            err.raw_os_error(),
            Some(28),
            "expected ENOSPC (28) from /dev/full, got {err:?}"
        );
        assert!(
            staging_seen_by_writer.is_some(),
            "the staging file did not exist when the write ran, so this test never \
             exercised the create-succeeds-then-write-fails shape"
        );

        // (3) Atomicity: the target is untouched, because only the rename ever
        // touches it and the failure happened well before that.
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            ORIGINAL,
            "a failed write must leave the target byte-identical"
        );

        // (4) The leak itself: nothing but the target is left in the directory.
        assert_eq!(
            entry_names_any(dir.path()),
            vec!["target".to_string()],
            "atomic_write leaked its staging file after the write failed"
        );
    }

    /// Names in `dir`, sorted. Platform-neutral twin of `entry_names` (which is unix-only
    /// because the fixtures around it plant symlinks).
    fn entry_names_any(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
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

/// `read_regular_bytes` / `read_regular_text`: a link is followed, a file that is not a regular
/// file is refused without a read, and the bound is on the bytes read.
#[cfg(all(test, unix))]
mod bounded_read_tests {
    use super::special_file_fixtures::{make_fifo, within_deadline};
    use super::*;
    use tempfile::tempdir;

    fn refused_as_not_regular(r: Result<String, BoundedReadError>) -> bool {
        matches!(r, Err(BoundedReadError::NotRegular))
    }

    /// A FIFO with no writer: a blocking open or read never returns, so a regression is a hang,
    /// which `within_deadline` reports as a failure.
    #[test]
    fn a_fifo_is_refused_not_read() {
        let dir = tempdir().unwrap();
        let fifo = dir.path().join("pipe");
        make_fifo(&fifo);
        let r = within_deadline("reading a FIFO", move || {
            read_regular_text(&fifo, MAX_CONFIG_FILE_BYTES)
        });
        assert!(refused_as_not_regular(r));
    }

    /// A link to a FIFO is followed to the FIFO, then refused.
    #[test]
    fn a_link_to_a_fifo_is_refused_not_read() {
        let dir = tempdir().unwrap();
        let fifo = dir.path().join("pipe");
        make_fifo(&fifo);
        let link = dir.path().join("link.toml");
        std::os::unix::fs::symlink(&fifo, &link).unwrap();
        let r = within_deadline("reading a link to a FIFO", move || {
            read_regular_text(&link, MAX_CONFIG_FILE_BYTES)
        });
        assert!(refused_as_not_regular(r));
    }
    /// The git pointer reader answers `None` for everything but a regular file of at most
    /// `MAX_GIT_POINTER_BYTES`, and never holds on a FIFO.
    #[test]
    fn read_git_pointer_reads_a_regular_file_and_refuses_the_rest() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("HEAD");
        std::fs::write(&file, "ref: refs/heads/main\n").unwrap();
        assert_eq!(
            read_git_pointer(&file).as_deref(),
            Some("ref: refs/heads/main\n")
        );
        assert_eq!(read_git_pointer(&dir.path().join("absent")), None);

        let fifo = dir.path().join("pipe");
        make_fifo(&fifo);
        assert_eq!(
            within_deadline("read_git_pointer on a FIFO", move || read_git_pointer(
                &fifo
            )),
            None
        );

        let big = std::fs::OpenOptions::new().write(true).open(&file).unwrap();
        big.set_len(MAX_GIT_POINTER_BYTES + 1).unwrap();
        drop(big);
        assert_eq!(read_git_pointer(&file), None, "over the limit");
    }

    /// `/dev/zero` never ends. It is refused on its type, and even if it were not, the read is
    /// `take`n at the cap, so this cannot allocate more than the cap plus one byte.
    #[test]
    fn a_link_to_dev_zero_is_refused_and_never_read_unbounded() {
        let dir = tempdir().unwrap();
        let link = dir.path().join("zero.toml");
        std::os::unix::fs::symlink("/dev/zero", &link).unwrap();
        let r = within_deadline("reading a link to /dev/zero", move || {
            read_regular_text(&link, MAX_CONFIG_FILE_BYTES)
        });
        assert!(refused_as_not_regular(r));
    }

    #[test]
    fn a_directory_and_a_link_to_one_are_refused() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real-dir");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("dir-link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        for p in [&real, &link] {
            let r = read_regular_text(p, MAX_CONFIG_FILE_BYTES);
            assert!(refused_as_not_regular(r), "{}", p.display());
        }
    }

    #[test]
    fn a_link_to_a_regular_file_is_read() {
        let dir = tempdir().unwrap();
        let real = dir.path().join("real.toml");
        std::fs::write(&real, "name = \"x\"\n").unwrap();
        let link = dir.path().join("link.toml");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(
            read_regular_text(&link, MAX_CONFIG_FILE_BYTES).unwrap(),
            "name = \"x\"\n"
        );
    }

    #[test]
    fn the_cap_is_exact_at_the_limit_and_one_byte_over() {
        let dir = tempdir().unwrap();
        let at = dir.path().join("at");
        let over = dir.path().join("over");
        std::fs::write(&at, "a".repeat(64)).unwrap();
        std::fs::write(&over, "a".repeat(65)).unwrap();
        assert_eq!(read_regular_text(&at, 64).unwrap().len(), 64);
        assert!(matches!(
            read_regular_text(&over, 64),
            Err(BoundedReadError::TooLarge { max: 64 })
        ));
    }

    /// A `/proc` file is a regular file that reports length 0 and returns kilobytes. This proves
    /// the check AFTER the read: a file whose reported length passes the pre-check but whose
    /// bytes exceed the cap is still `TooLarge`. It does NOT prove the bound on the read itself
    /// (`take(max + 1)`): with `take(u64::MAX)` this file is small enough to be read whole and the
    /// post-read check still refuses it. That bound is proved by
    /// `the_read_itself_stops_one_byte_past_the_cap_and_never_pulls_more`, which uses a source
    /// that fails if more than `max + 1` bytes are pulled.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_file_reporting_length_zero_is_refused_when_its_bytes_exceed_the_cap() {
        let path = Path::new("/proc/self/status");
        let meta = std::fs::metadata(path).unwrap();
        assert!(meta.file_type().is_file());
        assert_eq!(meta.len(), 0, "premise: /proc reports length 0");
        assert!(
            std::fs::read(path).unwrap().len() > 16,
            "premise: it returns more than the cap used below"
        );
        assert!(matches!(
            read_regular_bytes(path, 16),
            Err(BoundedReadError::TooLarge { max: 16 })
        ));
    }

    #[test]
    fn a_missing_path_and_a_dangling_link_are_not_found_and_other_failures_are_not() {
        let dir = tempdir().unwrap();
        let gone = dir.path().join("gone.toml");
        assert!(read_regular_text(&gone, 64).unwrap_err().is_not_found());
        let dangling = dir.path().join("dangling.toml");
        std::os::unix::fs::symlink(&gone, &dangling).unwrap();
        assert!(read_regular_text(&dangling, 64).unwrap_err().is_not_found());
        assert!(!BoundedReadError::NotRegular.is_not_found());
        assert!(!BoundedReadError::TooLarge { max: 1 }.is_not_found());
    }

    #[test]
    fn invalid_utf8_is_an_error_for_text_and_bytes_for_bytes() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("bin");
        std::fs::write(&p, [0xff, 0xfe, 0x41]).unwrap();
        assert!(matches!(
            read_regular_text(&p, 64),
            Err(BoundedReadError::Io(e)) if e.kind() == std::io::ErrorKind::InvalidData
        ));
        assert_eq!(read_regular_bytes(&p, 64).unwrap(), vec![0xff, 0xfe, 0x41]);
    }

    /// The message names the reason and nothing from inside the file or its path.
    #[test]
    fn the_refusal_text_carries_no_path_and_no_content() {
        let dir = tempdir().unwrap();
        let secret = dir.path().join("SECRET_PATH_QX7");
        std::fs::create_dir(&secret).unwrap();
        let shown = read_regular_text(&secret, 64).unwrap_err().to_string();
        assert!(!shown.contains("SECRET_PATH_QX7"), "got: {shown}");
    }

    /// A source that supplies bytes without end and fails the moment more than `fail_after`
    /// have been pulled from it. A bound that is really on the READER ends the read first with
    /// `TooLarge`; one that is only checked afterwards (`take(u64::MAX)`) reaches the failure.
    struct EndlessThenFails {
        pulled: u64,
        fail_after: u64,
    }

    impl std::io::Read for EndlessThenFails {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.pulled >= self.fail_after {
                return Err(std::io::Error::other("read past the bound"));
            }
            let n = (self.fail_after - self.pulled).min(buf.len() as u64) as usize;
            buf[..n].fill(b'a');
            self.pulled += n as u64;
            Ok(n)
        }
    }

    #[test]
    fn the_read_itself_stops_one_byte_past_the_cap_and_never_pulls_more() {
        let max = 100;
        let r = read_bounded(
            EndlessThenFails {
                pulled: 0,
                fail_after: max + 1,
            },
            max,
        );
        assert!(
            matches!(r, Err(BoundedReadError::TooLarge { max: 100 })),
            "the source fails after max+1 bytes, so a read that stopped at max+1 never sees it; got {r:?}"
        );
        let exact = read_bounded(
            EndlessThenFails {
                pulled: 0,
                fail_after: max,
            },
            max,
        );
        assert!(
            matches!(exact, Err(BoundedReadError::Io(_))),
            "control: a source that fails inside the allowance surfaces its error; got {exact:?}"
        );
    }

    /// `open(2)` of a socket fails `ENXIO`; it is the same refusal as any other non-file.
    #[test]
    fn a_socket_is_refused_as_not_a_regular_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sock");
        let _listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let r = within_deadline("reading a socket", move || {
            read_regular_text(&path, MAX_CONFIG_FILE_BYTES)
        });
        assert!(refused_as_not_regular(r));
    }

    /// A panic inside the closure is reported as that panic, not as a hang.
    #[test]
    #[should_panic(expected = "the closure's own failure")]
    fn within_deadline_reraises_a_panic_from_the_closure() {
        within_deadline("a closure that panics", || -> () {
            panic!("the closure's own failure")
        });
    }
}
