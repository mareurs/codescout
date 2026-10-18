//! The read door: the one way librarian code reads the CONTENT of a file that a repository or the
//! catalog names.
//!
//! **Invariant.** *The librarian hands the bytes of a file that a repository or the catalog names
//! (to a caller, to the chunk table or to an embedder) only if the file, AS OPENED, lies inside
//! the root [`crate::librarian::util::door_root`] names for its spelling and is not on the read
//! deny-list. When no root can be named, it reads only a spelling that crosses no symbolic link
//! at all.*
//!
//! The root is the WRITE door's root, so the librarian reads only where it would write: there is
//! one notion of "inside", decided in one function, and the seven-round layout table of the write
//! door (`door_table` in `tools/link_dir_tests.rs`) carries over. The one deliberate difference
//! from the write door is the leaf rule: a leaf link that resolves INSIDE is read, because its
//! bytes are the repository's own; the write door refuses every leaf link because a write would
//! go through it. The table that pins every read site to this one verdict is
//! `tools/read_door_tests.rs`.
//!
//! **The deny-list is a second layer, always on.** [`validate_read_path`] is called with
//! `PathSecurityConfig::default()`, NOT the project's config: `[security] profile = "root"` lives
//! in a file a repository (or the agent) can write, so honouring it would let a repository switch
//! the layer off. It only matters where a judged root contains a credentials location (a managed
//! root that is a home directory); containment refuses every other case first.
//!
//! **Opened once, then judged.** [`ReadDoor::bytes`] opens the path (following its links once),
//! requires a regular file, and judges the OPENED FILE's location: on Linux the descriptor's own
//! path (`/proc/self/fd/N`, the kernel's record of where the opened inode lives, which no later
//! swap of a link in the spelling changes); elsewhere, or with no `/proc`, the canonicalized
//! spelling. It then requires the judged path to name the opened inode by its OWN `st_dev` and
//! `st_ino` (`symlink_metadata`: a canonical path is never a link, so following one there was
//! the hole review round 1 found, a leaf swapped inside for the judgement and back for the check),
//! and reads from the descriptor it opened, never reopening by path, with a fallible allocation.
//! On Linux that closes the race between the check and the read. On the fallback path it is
//! closed for a swap of the LEAF, not for a swap of a PARENT directory to an inside directory for
//! the judgement and back afterwards (the canonical path then resolves through the restored link
//! to the opened inode). What is not covered anywhere: a HARD link to an outside file placed
//! inside the repository (same inode, inside path; `git checkout` cannot create one, and a process
//! that can create one can already read the target) and a bind mount the operator made. A
//! concurrent change of DIRECTORY links in the spelling can at worst make the judging root the
//! confinement root instead of a nested repository; the bytes are then still inside what the
//! caller manages. **macOS takes the fallback path (`F_GETPATH` is not wired) and was not run.
//! Windows has no stable file index in `std`, so there the door judges and then reads with a race
//! window, and it is unverified (decision D9).** A refusal names the spelling and the repository
//! root, never the resolved location outside it.
//!
//! **Bytes the catalog already stored** (chunk text, a title) were read by an older walk;
//! [`ReadDoor::refusal`] asks the same question of the file as it is now, without opening it, so
//! a site that returns stored text can withhold it. A dangling leaf link is refused there: where
//! it pointed cannot be judged.
//!
//! Every production read of file content in `src/librarian` goes through this module; the source
//! gate `no_librarian_production_read_bypasses_the_read_door` (below) fails when one does not, and
//! lists, with a reason each, the few reads of files no repository names.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use crate::librarian::tools::LibrarianRecoverableError;

/// Why a read through the door did not return bytes.
#[derive(Debug)]
pub(crate) enum ReadError {
    /// The door refused the file: it resolves outside its repository, it is on the deny-list, it
    /// is not a regular file, or it changed between the open and the check. A
    /// `LibrarianRecoverableError` naming the path and the reason, never the content.
    Refused(anyhow::Error),
    /// The OS refused to open or read it (missing, permission, not UTF-8): what `std::fs` said.
    Io(std::io::Error),
}

impl ReadError {
    /// The error as an `anyhow::Error`, keeping a refusal downcastable to
    /// `LibrarianRecoverableError` (so it stays recoverable at the tool boundary).
    pub(crate) fn into_anyhow(self) -> anyhow::Error {
        match self {
            ReadError::Refused(e) => e,
            ReadError::Io(e) => e.into(),
        }
    }

    pub(crate) fn is_refusal(&self) -> bool {
        matches!(self, ReadError::Refused(_))
    }
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Refused(e) => write!(f, "{e:#}"),
            ReadError::Io(e) => write!(f, "{e}"),
        }
    }
}

/// The door, for one caller's roots. Build one per tool call (or per walk) and reuse it: it
/// remembers its verdict for each `(spelling, resolved path)` it has judged. The judgement walks
/// the spelling's ancestors and asks the deny-list (~40 canonicalizations), so a `doctor` run that
/// reads every row several times pays for it once per file; the open-then-verify read itself is
/// never cached.
#[derive(Debug)]
pub(crate) struct ReadDoor {
    roots: Vec<PathBuf>,
    judged: parking_lot::Mutex<HashMap<(PathBuf, PathBuf), Option<String>>>,
}

impl ReadDoor {
    /// `roots` are the caller's managed roots (`tools::managed_roots`), or the walk's root, or
    /// empty where the caller has none: the same input the write door takes.
    pub(crate) fn new(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            judged: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    /// The file's bytes, if the door admits it (see the module doc for the order of the steps).
    pub(crate) fn bytes(&self, path: &Path) -> Result<Vec<u8>, ReadError> {
        read_verified(path, |real| self.judge(path, real))
    }

    /// [`Self::bytes`] as UTF-8 text; invalid UTF-8 is `ReadError::Io(InvalidData)`, as
    /// `std::fs::read_to_string` reports it.
    pub(crate) fn text(&self, path: &Path) -> Result<String, ReadError> {
        utf8(self.bytes(path)?)
    }

    /// For a read-modify-write verb: the write door's leaf rule first (a leaf link is never
    /// written through, so it is not read either), then [`Self::text`]. Refusing BEFORE the read
    /// is what keeps an error built from the content (a parse error, a list of headings) from
    /// reaching the caller for a file the write would refuse anyway.
    pub(crate) fn text_for_write(&self, path: &Path) -> Result<String, ReadError> {
        crate::librarian::util::refuse_write_through_symlink(path).map_err(ReadError::Refused)?;
        self.text(path)
    }

    /// Would the door read `path` now? `None` when it would. For a site that returns bytes the
    /// catalog already holds (stored chunk text, a stored title) without opening the file. A
    /// missing plain path is judged where it would be; a dangling leaf link is refused.
    pub(crate) fn refusal(&self, path: &Path) -> Option<anyhow::Error> {
        let real = match std::fs::canonicalize(path) {
            Ok(r) => r,
            Err(_) => {
                let is_link = std::fs::symlink_metadata(path)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
                if is_link {
                    return Some(refused(
                        path,
                        "it is a symbolic link whose target does not exist, so where it pointed \
                         cannot be judged",
                    ));
                }
                crate::librarian::util::resolve_through_existing_ancestor(path)
            }
        };
        self.judge(path, &real).err()
    }

    /// The judgement, on the path the OS resolved (`real`), against the root the SPELLING names.
    /// Remembered per `(spelling, real)` for the door's lifetime (one tool call or walk), so a run
    /// that reads a file several times judges it once; the open-then-verify steps around it are
    /// repeated on every read.
    fn judge(&self, spelled: &Path, real: &Path) -> anyhow::Result<()> {
        let key = (spelled.to_path_buf(), real.to_path_buf());
        let cached = self.judged.lock().get(&key).cloned();
        let why = match cached {
            Some(why) => why,
            None => {
                let why = self.verdict(spelled, real);
                self.judged.lock().insert(key, why.clone());
                why
            }
        };
        match why {
            Some(why) => Err(refused(spelled, &why)),
            None => Ok(()),
        }
    }

    /// `None` when the door admits `real` for `spelled`, else the reason it refuses.
    fn verdict(&self, spelled: &Path, real: &Path) -> Option<String> {
        match crate::librarian::util::door_root(&self.roots, spelled) {
            Some(root) => {
                let root_real = crate::librarian::util::resolve_through_existing_ancestor(&root);
                let climbs = real.components().any(|c| matches!(c, Component::ParentDir));
                if climbs || !real.starts_with(&root_real) {
                    // The repository root is named; the resolved location outside it is not (it would
                    // tell the caller that the target exists, and where).
                    return Some(format!(
                        "it resolves outside the repository root `{}`",
                        root_real.display()
                    ));
                }
            }
            None => {
                // No managed root contains the spelling and no repository is above it, so there
                // is nothing to judge a link against: read only a spelling that crosses no link.
                // Compared on `real`, the path the inode check ties to the opened file, not on a
                // fresh `lstat` that a swap could answer differently.
                let lexical: PathBuf = spelled.components().collect();
                if real != lexical {
                    return Some(
                        "it reaches its file through a symbolic link, and no managed root or \
                         repository contains it to judge the link against"
                            .to_string(),
                    );
                }
            }
        }
        if deny_listed(real) {
            return Some(DENIED.to_string());
        }
        None
    }
}

/// The read deny-list, as `read_file` applies it, with the DEFAULT profile (see the module doc).
/// A path that is not valid UTF-8 is checked through its lossy spelling, which matches no entry;
/// with a non-UTF-8 home the layer is therefore blind (containment still applies).
fn deny_listed(real: &Path) -> bool {
    crate::util::path_security::validate_read_path(
        &real.to_string_lossy(),
        None,
        &crate::util::path_security::PathSecurityConfig::default(),
    )
    .is_err()
}
/// The door for a catalog-level reader, which takes no `ToolContext`: the managed roots the
/// calling tool armed on the catalog (`Catalog::arm_write_roots`), or none, in which case each
/// path is judged against the repository it spells.
pub(crate) fn for_catalog(cat: &crate::librarian::catalog::Catalog) -> ReadDoor {
    ReadDoor::new(cat.write_roots())
}
/// The door for a reader that holds only a PROJECT root (a project config file, a policy file):
/// the active-project part of `tools::managed_roots`, the project's git root first and then the
/// project itself, so a file such a reader names is judged as every other site would judge it.
pub(crate) fn for_project(project_root: &Path) -> ReadDoor {
    let mut roots = Vec::new();
    if let Some(git) = crate::librarian::current_project::lookup_git_root(project_root) {
        roots.push(git);
    }
    if !roots.iter().any(|r| r == project_root) {
        roots.push(project_root.to_path_buf());
    }
    ReadDoor::new(roots)
}

/// Read an AGENT-named path (`doc(augment)`'s `params_path`): the deny-list only, as `read_file`
/// applies it, with the same open-then-verify read. Not a containment check, by decision D5: the
/// documented use is a JSON file the agent wrote to a scratch or temp directory.
pub(crate) fn read_agent_named_text(path: &Path) -> Result<String, ReadError> {
    utf8(read_verified(path, |real| {
        if deny_listed(real) {
            return Err(refused(path, DENIED));
        }
        Ok(())
    })?)
}

/// The deny-list refusal, naming no location.
const DENIED: &str = "it resolves to a protected location on the read deny-list";

fn utf8(bytes: Vec<u8>) -> Result<String, ReadError> {
    String::from_utf8(bytes).map_err(|e| {
        ReadError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.utf8_error(),
        ))
    })
}

fn refused(path: &Path, why: &str) -> anyhow::Error {
    LibrarianRecoverableError::with_hint(
        format!("refusing to read `{}`: {why}", path.display()),
        "Nothing was read. The librarian reads a file only where it would write it, inside the \
         repository its path belongs to, and never a location on the read deny-list. If the path \
         is a symbolic link out of the repository, read the target under its own path with your \
         file tools, or replace the link with a regular file.",
    )
}

/// Open once, check the opened file, judge the canonical path, tie the two together, read from
/// the descriptor. `judge` sees the canonical path and decides.
fn read_verified(
    path: &Path,
    judge: impl Fn(&Path) -> anyhow::Result<()>,
) -> Result<Vec<u8>, ReadError> {
    use std::io::Read;
    // Before opening: opening a FIFO blocks and a device never ends, so a non-regular target is
    // refused without an open. The opened handle is checked again below.
    let pre = std::fs::metadata(path).map_err(ReadError::Io)?;
    if !pre.is_file() {
        return Err(ReadError::Refused(refused(
            path,
            "it is not a regular file",
        )));
    }
    let mut file = open_for_read(path).map_err(ReadError::Io)?;
    let opened = file.metadata().map_err(ReadError::Io)?;
    if !opened.is_file() {
        return Err(ReadError::Refused(refused(
            path,
            "it is not a regular file",
        )));
    }
    #[cfg(test)]
    tests::between_open_and_check(path);
    // The path judged is the DESCRIPTOR's own where the kernel can name it (Linux): the opened
    // file's real location, whatever the spelling's links do after the open. Elsewhere the
    // spelling is canonicalized and tied back to the opened inode below.
    let real = match descriptor_path(&file) {
        Some(p) => p,
        None => std::fs::canonicalize(path).map_err(|_| changed(path))?,
    };
    judge(&real).map_err(ReadError::Refused)?;
    #[cfg(test)]
    tests::after_judge(path);
    same_file(&opened, &real).map_err(|_| changed(path))?;
    // Fallible, as `std::fs::read` is: a file too large to hold (a sparse file can claim any
    // size) is an error, never an allocation failure that aborts the process.
    let mut buf = Vec::new();
    let len = usize::try_from(opened.len()).unwrap_or(usize::MAX);
    buf.try_reserve_exact(len).map_err(|e| {
        ReadError::Io(std::io::Error::new(
            std::io::ErrorKind::OutOfMemory,
            format!(
                "the file is too large to read ({} bytes): {e}",
                opened.len()
            ),
        ))
    })?;
    file.read_to_end(&mut buf).map_err(ReadError::Io)?;
    Ok(buf)
}

fn changed(path: &Path) -> ReadError {
    ReadError::Refused(refused(
        path,
        "it changed while it was being opened (a link was replaced between the open and the \
         check)",
    ))
}

/// `O_NONBLOCK` so a FIFO swapped in after the `metadata` check cannot block the open; it has no
/// effect on reading a regular file.
#[cfg(unix)]
fn open_for_read(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
}

#[cfg(not(unix))]
fn open_for_read(path: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::open(path)
}

/// The path the kernel records for an open descriptor (`/proc/self/fd/N`), or `None` where it
/// cannot say (no `/proc`, a non-path descriptor, another OS).
#[cfg(target_os = "linux")]
fn descriptor_path(file: &std::fs::File) -> Option<PathBuf> {
    use std::os::fd::AsRawFd;
    #[cfg(test)]
    if tests::fd_path_disabled() {
        return None;
    }
    let p = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd())).ok()?;
    p.is_absolute().then_some(p)
}

/// macOS (`F_GETPATH`) and the other platforms are not wired: the canonicalized spelling is
/// judged and tied to the opened inode by `same_file`, which a swap of a PARENT directory between
/// the check and the inode comparison can still defeat (unverified, stated in the module doc).
#[cfg(not(target_os = "linux"))]
fn descriptor_path(_file: &std::fs::File) -> Option<PathBuf> {
    None
}

/// Does `real` still name the inode that was opened? `real` is canonical, so it is never a link:
/// its OWN inode is compared (`symlink_metadata`). Following a link here (`metadata`) let a leaf
/// swapped to an inside file for the judgement and back to the outward link for this check pass
/// (review round 1, F2).
#[cfg(unix)]
pub(crate) fn same_file(opened: &std::fs::Metadata, real: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let now = std::fs::symlink_metadata(real)?;
    if (now.dev(), now.ino()) != (opened.dev(), opened.ino()) {
        anyhow::bail!("not the opened file");
    }
    Ok(())
}

/// Windows: `std` has no stable file index, so nothing ties the judged path to the opened file and
/// the judge-then-read window stays open. Unverified (decision D9).
#[cfg(not(unix))]
pub(crate) fn same_file(_opened: &std::fs::Metadata, _real: &Path) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn canon(p: &Path) -> PathBuf {
        std::fs::canonicalize(p).unwrap()
    }
    /// A test's action between the door's open and its check (`read_verified`).
    type Hook = Box<dyn Fn(&Path)>;

    std::thread_local! {
        /// A test's action between the door's open and its check (`read_verified`): the window a
        /// concurrent link swap would use.
        static BETWEEN: std::cell::RefCell<Option<Hook>> =
            const { std::cell::RefCell::new(None) };
    }

    pub(super) fn between_open_and_check(path: &Path) {
        BETWEEN.with(|h| {
            if let Some(f) = h.borrow().as_ref() {
                f(path)
            }
        });
    }

    std::thread_local! {
        /// A test's action between the judgement and the inode check: the window for a swap
        /// BACK, after the judge was shown an inside path.
        static AFTER_JUDGE: std::cell::RefCell<Option<Hook>> =
            const { std::cell::RefCell::new(None) };
        /// Pretend the platform cannot name an open descriptor's path, so the fallback (canonicalize
        /// then compare inodes) is the one under test.
        static NO_FD_PATH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    pub(super) fn after_judge(path: &Path) {
        AFTER_JUDGE.with(|h| {
            if let Some(f) = h.borrow().as_ref() {
                f(path)
            }
        });
    }

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(super) fn fd_path_disabled() -> bool {
        NO_FD_PATH.with(|c| c.get())
    }

    fn set_hooks(between: Option<Hook>, after: Option<Hook>) {
        BETWEEN.with(|h| *h.borrow_mut() = between);
        AFTER_JUDGE.with(|h| *h.borrow_mut() = after);
    }

    /// A repository, an outside directory holding `key.md` (the secret), and `root/notes.md`.
    #[cfg(unix)]
    fn flip_flop_fixture() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let base = canon(t.path());
        let root = base.join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(base.join("out")).unwrap();
        std::fs::write(base.join("out/key.md"), "FAKE-KEY-MARKER").unwrap();
        let outside = base.join("out/key.md");
        (t, root, outside, base)
    }

    #[cfg(unix)]
    fn assert_refused_without_the_secret(got: Result<Vec<u8>, ReadError>, what: &str) {
        match got {
            Err(ReadError::Refused(_)) => {}
            Ok(b) => panic!(
                "{what}: the door returned {:?}, the bytes of the file it opened, after judging \
                 another",
                String::from_utf8_lossy(&b)
            ),
            Err(other) => panic!("{what}: expected a refusal, got {other:?}"),
        }
    }

    /// Review round 1, F2: open while the leaf links OUTSIDE (the descriptor holds the outside
    /// inode); swap the leaf to an inside regular file, so the judged canonical path is inside and
    /// admitted; swap it BACK to the outward link before the inode check. A check that follows
    /// links (`metadata`) then finds the outside inode again and admits the bytes it opened.
    #[cfg(unix)]
    fn leaf_flip_flop(no_fd_path: bool) -> Result<Vec<u8>, ReadError> {
        let (_t, root, outside, _base) = flip_flop_fixture();
        let link = root.join("notes.md");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let o2 = outside.clone();
        set_hooks(
            Some(Box::new(|p: &Path| {
                std::fs::remove_file(p).unwrap();
                std::fs::write(p, "inside").unwrap();
            })),
            Some(Box::new(move |p: &Path| {
                std::fs::remove_file(p).unwrap();
                std::os::unix::fs::symlink(&o2, p).unwrap();
            })),
        );
        NO_FD_PATH.with(|c| c.set(no_fd_path));
        let got = ReadDoor::new(vec![root.clone()]).bytes(&link);
        NO_FD_PATH.with(|c| c.set(false));
        set_hooks(None, None);
        got
    }

    #[cfg(unix)]
    #[test]
    fn a_leaf_swapped_inside_for_the_judgement_and_back_is_refused() {
        assert_refused_without_the_secret(leaf_flip_flop(false), "descriptor path");
    }

    /// The same flip-flop where the platform cannot name an open descriptor's path: the fallback
    /// compares the opened inode with the canonical path's OWN inode (`symlink_metadata`), and a
    /// link there is never the opened file.
    #[cfg(unix)]
    #[test]
    fn a_leaf_swapped_inside_and_back_is_refused_by_the_fallback_too() {
        assert_refused_without_the_secret(leaf_flip_flop(true), "fallback");
    }

    /// The parent-directory form of the flip-flop, which an inode check on the canonical path
    /// cannot catch even with `symlink_metadata` (the leaf itself is never a link): the spelling
    /// is `root/d/key.md`; `d` is a link to the outside directory when the door opens; a real
    /// directory holding an inside `key.md` when it canonicalizes and judges; the outward link
    /// again at the inode check, so `root/d/key.md` resolves through it to the opened inode. On
    /// Linux the door judges the DESCRIPTOR's own path (`/proc/self/fd`), which is the opened
    /// file's real location whatever the spelling does afterwards.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_parent_directory_swapped_inside_for_the_judgement_and_back_is_refused() {
        let (_t, root, _outside, base) = flip_flop_fixture();
        let d = root.join("d");
        std::os::unix::fs::symlink(base.join("out"), &d).unwrap();
        let out = base.join("out");
        let (d1, d2) = (d.clone(), d.clone());
        let o2 = out.clone();
        set_hooks(
            Some(Box::new(move |_p: &Path| {
                std::fs::remove_file(&d1).unwrap();
                std::fs::create_dir(&d1).unwrap();
                std::fs::write(d1.join("key.md"), "inside").unwrap();
            })),
            Some(Box::new(move |_p: &Path| {
                std::fs::remove_dir_all(&d2).unwrap();
                std::os::unix::fs::symlink(&o2, &d2).unwrap();
            })),
        );
        let got = ReadDoor::new(vec![root.clone()]).bytes(&d.join("key.md"));
        set_hooks(None, None);
        assert_refused_without_the_secret(got, "parent flip-flop");
    }

    /// Review round 1, F4: a refusal names the SPELLING and the repository root, never the
    /// resolved location outside it (which would tell the caller that the target exists and
    /// where).
    #[cfg(unix)]
    #[test]
    fn a_refusal_names_the_spelling_and_the_root_never_the_outside_target() {
        let (_t, root, outside, _base) = flip_flop_fixture();
        std::fs::rename(&outside, outside.with_file_name("secret-place-id_rsa")).unwrap();
        let target = outside.with_file_name("secret-place-id_rsa");
        let link = root.join("n.md");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let door = ReadDoor::new(vec![root.clone()]);
        let texts = [
            door.text(&link).unwrap_err().to_string(),
            format!("{:#}", door.refusal(&link).unwrap()),
            format!("{:#}", ReadDoor::new(Vec::new()).refusal(&link).unwrap()),
        ];
        for t in &texts {
            assert!(t.contains("n.md"), "the spelling is named: {t}");
            assert!(
                !t.contains("secret-place"),
                "the outside target must not be named: {t}"
            );
        }
    }

    /// Review round 1, F3: a sparse file larger than any allocation. `std::fs::read`, which the
    /// walk used before the door, returns an error; `Vec::with_capacity` aborts the process. Run
    /// in a child process so an abort is a failed assertion rather than a dead test harness.
    #[cfg(unix)]
    #[test]
    fn a_file_too_large_to_hold_is_an_error_not_an_abort() {
        let t = tempfile::tempdir().unwrap();
        let root = canon(t.path()).join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let f = std::fs::File::create(root.join("big.md")).unwrap();
        if let Err(e) = f.set_len(1u64 << 62) {
            eprintln!("SKIPPED: this filesystem cannot hold a sparse 4 EiB file ({e})");
            return;
        }
        drop(f);
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "librarian::read_door::tests::huge_file_child",
                "--ignored",
                "--nocapture",
            ])
            .env("CS_HUGE_FILE", root.join("big.md"))
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success() && stdout.contains("HUGE-CHILD-RAN io-error"),
            "the door must return an error for an unallocatable file, the child said: {:?}\n{stdout}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "child of a_file_too_large_to_hold_is_an_error_not_an_abort"]
    fn huge_file_child() {
        let p = PathBuf::from(std::env::var("CS_HUGE_FILE").expect("run by the parent"));
        let root = p.parent().unwrap().to_path_buf();
        match ReadDoor::new(vec![root]).bytes(&p) {
            Err(ReadError::Io(_)) => println!("HUGE-CHILD-RAN io-error"),
            other => println!(
                "HUGE-CHILD-RAN unexpected {:?}",
                other.map(|v| v.len()).map_err(|e| e.to_string())
            ),
        }
    }

    /// The race the inode check exists for, made deterministic: the door opens a link that points
    /// OUTSIDE (the descriptor now holds the outside file), the link is then re-pointed at a file
    /// INSIDE, so the canonical path the door judges is inside and admitted. Only the inode check
    /// sees that the judged path is not the opened file; without it the outside bytes come back.
    #[cfg(unix)]
    #[test]
    fn a_link_swapped_between_the_open_and_the_check_is_refused() {
        let t = tempfile::tempdir().unwrap();
        let base = canon(t.path());
        let root = base.join("repo");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(base.join("out")).unwrap();
        std::fs::write(root.join("real.md"), "inside").unwrap();
        std::fs::write(base.join("out/key.md"), "FAKE-KEY-MARKER").unwrap();
        let link = root.join("notes.md");
        std::os::unix::fs::symlink(base.join("out/key.md"), &link).unwrap();
        let real = root.join("real.md");
        BETWEEN.with(|h| {
            *h.borrow_mut() = Some(Box::new(move |p: &Path| {
                std::fs::remove_file(p).unwrap();
                std::os::unix::fs::symlink(&real, p).unwrap();
            }))
        });
        let got = ReadDoor::new(vec![root.clone()]).bytes(&link);
        BETWEEN.with(|h| *h.borrow_mut() = None);
        match got {
            // Refused by the judgement of the descriptor's path (Linux) or by the inode check.
            Err(ReadError::Refused(_)) => {}
            Ok(b) => panic!(
                "the door returned {} bytes of the file it opened, judged on another",
                b.len()
            ),
            Err(other) => panic!("expected a refusal, got {other:?}"),
        }
        // Control: with nothing swapped, the in-repo file reads.
        assert_eq!(
            ReadDoor::new(vec![root.clone()])
                .text(&root.join("real.md"))
                .unwrap(),
            "inside"
        );
    }

    /// The inode check, through its seam: the opened file's metadata against a path that names a
    /// DIFFERENT file is refused, and against the same file is admitted. This is the step that
    /// closes the race between the check and the read; without it a link swapped after the open
    /// would be judged on one file while another is read.
    #[cfg(unix)]
    #[test]
    fn the_door_refuses_a_path_that_no_longer_names_the_opened_file() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.md");
        let b = t.path().join("b.md");
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        let opened = std::fs::File::open(&a).unwrap().metadata().unwrap();
        assert!(
            same_file(&opened, &a).is_ok(),
            "control: the same file is admitted"
        );
        assert!(
            same_file(&opened, &b).is_err(),
            "a path naming another inode is refused"
        );
    }

    /// Opening a FIFO blocks and a device never ends, so a target that is not a regular file is
    /// refused before it is opened; a directory (and a link to one) too.
    #[cfg(unix)]
    #[test]
    fn a_target_that_is_not_a_regular_file_is_refused_without_opening_it() {
        let t = tempfile::tempdir().unwrap();
        let root = canon(t.path());
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("dir")).unwrap();
        std::os::unix::fs::symlink(root.join("dir"), root.join("todir.md")).unwrap();
        let fifo = root.join("pipe.md");
        let c = std::ffi::CString::new(fifo.to_string_lossy().as_bytes()).unwrap();
        // SAFETY: a plain mkfifo(3) call on a NUL-terminated path in a test's own temp dir.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let door = ReadDoor::new(vec![root.clone()]);
        for p in [root.join("dir"), root.join("todir.md"), fifo] {
            match door.bytes(&p) {
                Err(ReadError::Refused(e)) => {
                    assert!(format!("{e:#}").contains("not a regular file"), "{e:#}")
                }
                other => panic!("{} must be refused, got {other:?}", p.display()),
            }
        }
    }

    /// With no managed root and no repository to name, there is nothing to judge a link against:
    /// the door reads only a spelling that crosses no link, leaf OR parent. (The write door judges
    /// only the leaf there, so this is the one place the read door is the stricter.)
    #[cfg(unix)]
    #[test]
    fn with_no_root_to_name_a_spelling_through_any_link_is_refused() {
        let t = tempfile::tempdir().unwrap();
        let base = canon(t.path());
        let p = base.join("p");
        let out = base.join("out");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("x.md"), "outside").unwrap();
        std::fs::write(p.join("plain.md"), "plain").unwrap();
        std::fs::write(p.join("real.md"), "real").unwrap();
        std::os::unix::fs::symlink(out.join("x.md"), p.join("leaf.md")).unwrap();
        std::os::unix::fs::symlink("real.md", p.join("inleaf.md")).unwrap();
        std::os::unix::fs::symlink(&out, p.join("dirlink")).unwrap();
        let door = ReadDoor::new(Vec::new());
        assert_eq!(door.text(&p.join("plain.md")).unwrap(), "plain");
        for refused in ["leaf.md", "inleaf.md", "dirlink/x.md"] {
            assert!(
                matches!(door.text(&p.join(refused)), Err(ReadError::Refused(_))),
                "{refused}"
            );
            assert!(door.refusal(&p.join(refused)).is_some(), "{refused}");
        }
    }

    /// The predicate for stored bytes: a dangling leaf link is refused (where it pointed cannot be
    /// judged); a missing PLAIN path is judged where it would be, and admitted inside its root.
    #[cfg(unix)]
    #[test]
    fn the_predicate_refuses_a_dangling_link_and_judges_a_missing_plain_path() {
        let t = tempfile::tempdir().unwrap();
        let root = canon(t.path());
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::os::unix::fs::symlink(root.join("gone.md"), root.join("dangling.md")).unwrap();
        let door = ReadDoor::new(vec![root.clone()]);
        assert!(door.refusal(&root.join("dangling.md")).is_some());
        assert!(door.refusal(&root.join("never-existed.md")).is_none());
    }

    // ---------------------------------------------------------------------------------------
    // The source gate.
    // ---------------------------------------------------------------------------------------

    /// Reads of files NO repository names, each with the reason it is not a door read. An entry
    /// names the file, a fragment unique to the line, and how many raw reads such lines hold; the
    /// gate requires exactly that many (a liveness floor: an entry whose read is gone fails too,
    /// and a second read added to an allowed line is one too many).
    const ALLOWED: &[(&str, &str, usize, &str)] = &[
        ("workspace.rs", "std::fs::read_to_string(path).with_context", 1,
         "the operator's global ~/.config/librarian/workspace.toml (or LIBRARIAN_WORKSPACE)"),
        ("workspace.rs", "std::fs::OpenOptions::new()", 1,
         "creates the default global config with create_new; opens for writing only"),
        ("session_registry.rs", "/proc/{pid}/stat", 1, "the kernel's process table"),
        ("session_registry.rs", "std::fs::read_to_string(&path)", 1,
         "session JSON files under the Claude profile directories, machine state"),
        ("catalog/audit/host.rs", "/etc/hostname", 1, "the host name"),
        ("catalog/audit/shard.rs", "std::fs::read_to_string(repo_root.join(\".gitattributes\"))", 1,
         "audit shard I/O is out of this fix's scope (filed separately)"),
        ("catalog/audit/shard.rs", "std::fs::OpenOptions::new()", 1,
         "audit shard I/O is out of this fix's scope (filed separately)"),
        ("catalog/audit/shard.rs", "std::fs::File::open(&dir)", 1,
         "audit shard I/O is out of this fix's scope: an fsync of a directory, no content"),
        ("catalog/audit/shard.rs", "std::fs::read_to_string(e.path())", 1,
         "audit shard I/O is out of this fix's scope (filed separately)"),
        ("tools/worktree.rs", "worktree_root.join(\".git\")", 1,
         "a worktree's `.git` pointer file, parsed for its gitdir"),
        ("tools/worktree.rs", "join(\"HEAD\")", 1,
         "the HEAD of the gitdir that pointer names, which lies outside the worktree by construction; only `ref: refs/heads/<name>` is parsed"),
        ("tools/doctor.rs", "join(\"cc_session_id\")", 1,
         "a gitignored per-session id file; only a session-id-shaped value is used"),
        ("tools/doctor.rs", "e.path().join(\"gitdir\")", 1,
         "git's own .git/worktrees/*/gitdir registration files"),
    ];

    /// Production code of a Rust source: every `#[cfg(test)]` item blanked, and every string,
    /// character literal and comment blanked, so a pattern is matched on CODE only and line
    /// numbers survive. Returns the blanked text and the names of `#[cfg(test)] mod x;`
    /// declarations (whose files are test code in full).
    fn production_code(src: &str) -> (String, Vec<String>) {
        let b: Vec<char> = src.chars().collect();
        let mut code: Vec<char> = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            let next = b.get(i + 1).copied();
            if c == '/' && next == Some('/') {
                while i < b.len() && b[i] != '\n' {
                    code.push(' ');
                    i += 1;
                }
            } else if c == '/' && next == Some('*') {
                let mut depth = 0usize;
                loop {
                    if i >= b.len() {
                        break;
                    }
                    if b[i] == '/' && b.get(i + 1) == Some(&'*') {
                        depth += 1;
                        code.extend([' ', ' ']);
                        i += 2;
                    } else if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                        depth -= 1;
                        code.extend([' ', ' ']);
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        code.push(if b[i] == '\n' { '\n' } else { ' ' });
                        i += 1;
                    }
                }
            } else if c == 'r'
                && (next == Some('"') || next == Some('#'))
                && !(i > 0 && (b[i - 1].is_alphanumeric() || b[i - 1] == '_'))
            {
                // A raw string: r"..." or r#"..."#.
                let mut j = i + 1;
                let mut hashes = 0;
                while j < b.len() && b[j] == '#' {
                    hashes += 1;
                    j += 1;
                }
                if j < b.len() && b[j] == '"' {
                    code.push('r');
                    code.extend(std::iter::repeat_n(' ', j - i));
                    i = j + 1;
                    'raw: while i < b.len() {
                        if b[i] == '"' && (1..=hashes).all(|k| b.get(i + k) == Some(&'#')) {
                            code.extend(std::iter::repeat_n(' ', hashes + 1));
                            i += hashes + 1;
                            break 'raw;
                        }
                        code.push(if b[i] == '\n' { '\n' } else { ' ' });
                        i += 1;
                    }
                } else {
                    code.push(c);
                    i += 1;
                }
            } else if c == '"' {
                code.push(' ');
                i += 1;
                while i < b.len() && b[i] != '"' {
                    if b[i] == '\\' {
                        code.push(' ');
                        i += 1;
                    }
                    if i < b.len() {
                        code.push(if b[i] == '\n' { '\n' } else { ' ' });
                        i += 1;
                    }
                }
                code.push(' ');
                i += 1;
            } else if c == '\'' {
                // A char literal ('x', '\n', '\'') or a lifetime ('a): a char literal closes
                // within a few characters, a lifetime does not.
                if next == Some('\\') {
                    let mut j = i + 2;
                    while j < b.len() && b[j] != '\'' {
                        j += 1;
                    }
                    code.extend(std::iter::repeat_n(' ', j.min(b.len() - 1) + 1 - i));
                    i = j + 1;
                } else if b.get(i + 2) == Some(&'\'') {
                    code.extend([' ', ' ', ' ']);
                    i += 3;
                } else {
                    code.push(c);
                    i += 1;
                }
            } else {
                code.push(c);
                i += 1;
            }
        }
        let code: String = code.into_iter().collect();
        // Blank every `#[cfg(test)]` item: up to its `;` or through its braced body.
        let mut out: Vec<char> = code.chars().collect();
        let mut test_mods = Vec::new();
        let needle: Vec<char> = "#[cfg(test)]".chars().collect();
        let mut i = 0;
        while i + needle.len() <= out.len() {
            if out[i..i + needle.len()] != needle[..] {
                i += 1;
                continue;
            }
            let start = i;
            let mut j = i + needle.len();
            let mut end = None;
            let mut depth = 0usize;
            let mut header = String::new();
            while j < out.len() {
                match out[j] {
                    ';' if depth == 0 => {
                        end = Some(j);
                        break;
                    }
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(j);
                            break;
                        }
                    }
                    ch => {
                        if depth == 0 {
                            header.push(ch);
                        }
                    }
                }
                j += 1;
            }
            let end = end.expect("a #[cfg(test)] item ends");
            let words: Vec<&str> = header.split_whitespace().collect();
            if out[end] == ';' {
                if let Some(pos) = words.iter().position(|w| *w == "mod") {
                    if let Some(name) = words.get(pos + 1) {
                        test_mods.push(name.to_string());
                    }
                }
            }
            for ch in &mut out[start..=end] {
                if *ch != '\n' {
                    *ch = ' ';
                }
            }
            i = end + 1;
        }
        (out.into_iter().collect(), test_mods)
    }

    const RAW_READS: &[&str] = &[
        "fs::read(",
        "fs::read_to_string(",
        "File::open(",
        "OpenOptions::new(",
        "tokio::fs::read",
        "use std::fs::read",
    ];

    /// Every production read of file content in `src/librarian` goes through this module. A raw
    /// read added anywhere else, or a second one in a file the allow-list names, turns this red;
    /// so does an allow-list entry whose read is gone. The door's own file is exempt, and must
    /// show its own raw open: that is the proof the scanner sees production code at all.
    #[test]
    fn no_librarian_production_read_bypasses_the_read_door() {
        let lib = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src/librarian"));
        let mut files: Vec<PathBuf> = walkdir::WalkDir::new(&lib)
            .into_iter()
            .filter_map(|e| e.ok())
            .map(|e| e.into_path())
            .filter(|p| p.extension().is_some_and(|x| x == "rs"))
            .collect();
        files.sort();
        // Pass 1: the files a `#[cfg(test)] mod x;` declares are test code in full.
        let mut test_files: std::collections::BTreeSet<PathBuf> = Default::default();
        for f in &files {
            let src = std::fs::read_to_string(f).unwrap();
            let (_, mods) = production_code(&src);
            let dir = if f.file_name().is_some_and(|n| n == "mod.rs") {
                f.parent().unwrap().to_path_buf()
            } else {
                f.with_extension("")
            };
            for m in mods {
                test_files.insert(dir.join(format!("{m}.rs")));
                test_files.insert(dir.join(&m).join("mod.rs"));
            }
        }
        let mut hits: Vec<(String, String)> = Vec::new();
        let mut door_hits = 0usize;
        for f in &files {
            if test_files.contains(f) {
                continue;
            }
            let rel = f
                .strip_prefix(&lib)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let src = std::fs::read_to_string(f).unwrap();
            let (code, _) = production_code(&src);
            for (n, line) in code.lines().enumerate() {
                // Every OCCURRENCE counts, not every line: a second raw read appended to an
                // allowed line is a second read.
                let occurrences: usize = RAW_READS.iter().map(|p| line.matches(p).count()).sum();
                if occurrences == 0 {
                    continue;
                }
                if rel == "read_door.rs" {
                    door_hits += occurrences;
                    continue;
                }
                let original = src.lines().nth(n).unwrap_or("").trim().to_string();
                for _ in 0..occurrences {
                    hits.push((rel.clone(), original.clone()));
                }
            }
        }
        assert!(
            door_hits >= 2,
            "the scanner must see the door's own open (and its non-Unix twin): {door_hits}"
        );
        let mut problems = Vec::new();
        let mut matched = vec![false; hits.len()];
        for (file, fragment, count, why) in ALLOWED {
            assert!(!why.is_empty(), "every allow-list entry carries a reason");
            let mine: Vec<usize> = hits
                .iter()
                .enumerate()
                .filter(|(_, (f, l))| f == file && l.contains(fragment))
                .map(|(i, _)| i)
                .collect();
            if mine.len() != *count {
                problems.push(format!(
                    "allow-list entry {file} `{fragment}`: expected {count} read(s), found {}",
                    mine.len()
                ));
            }
            for i in mine {
                matched[i] = true;
            }
        }
        for (i, (file, line)) in hits.iter().enumerate() {
            if !matched[i] {
                problems.push(format!(
                    "{file}: `{line}` reads a file outside the read door; route it through \
                     `read_door` (or, for a file no repository names, add it to ALLOWED with a \
                     reason)"
                ));
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }

    /// The gate's lexer, on the shapes that would make it lie: a raw read inside a string, a
    /// comment or a test module is NOT production code, and one in code is.
    #[test]
    fn the_gate_sees_code_and_not_strings_comments_or_test_items() {
        let src = r##"
fn prod() { let x = std::fs::read_to_string(p); }
// std::fs::read_to_string(in_a_comment)
fn s() { let s = "std::fs::read(in_a_string)"; let r = r#"File::open(raw)"#; let c = '{'; }
/* std::fs::read(block) */
#[cfg(test)]
mod tests { fn t() { let _ = std::fs::read(p); let b = '}'; } }
#[cfg(test)]
mod declared_tests;
fn after() -> &'static str { std::fs::File::open(q); "" }
"##;
        let (code, mods) = production_code(src);
        let hit: Vec<usize> = code
            .lines()
            .enumerate()
            .filter(|(_, l)| RAW_READS.iter().any(|p| l.contains(p)))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(hit, vec![1, 9], "{code}");
        assert_eq!(mods, vec!["declared_tests".to_string()]);
    }
}
