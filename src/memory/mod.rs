//! Markdown-based persistent memory store (mirrors Serena's memory system).
//!
//! Memories are stored as `.md` files in `.codescout/memories/`.
//! They are organized hierarchically via path-like topics:
//! e.g. "debugging/async-patterns" → `.codescout/memories/debugging/async-patterns.md`

pub mod anchors;
pub mod classify;
pub mod filter;
pub mod guarded;
pub mod hash;
pub mod semantic_store;
pub mod sqlite_semantic_store;
mod write_transaction;

use crate::tools::RecoverableError;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// The shared shrink predicate's floor and report type, re-exported so callers
/// of this module keep their existing paths.
///
/// This module used to define its own copy of both, on the argument that the
/// two floors answer different questions — that one sized for just-created
/// frontmatter shells, this one for stub memories — and that agreeing today was
/// no reason to couple them. That reasoning was about the *floor*, and as far
/// as it went it was fine; it just did not govern the part that mattered. The
/// **predicate** had been duplicated along with the floor, three times across
/// the codebase, and on 2026-08-28 that cost 1047 lines of a tracker: a
/// line-truncating write slipped under all three byte-only tests, and fixing
/// any one copy would have left the other two. Divergence in a safety check is
/// not a property it can afford.
///
/// The floors may genuinely want to differ some day. If that day comes, give
/// [`crate::util::shrink_guard::check`] a floor parameter — do not fork the
/// predicate again.
pub use crate::util::shrink_guard::{ShrinkReport, SHRINK_GUARD_MIN_BYTES};

/// Per-project memory store.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    memories_dir: PathBuf,
}

impl MemoryStore {
    /// Open (or create) the memory store for a project root.
    pub fn open(project_root: &Path) -> Result<Self> {
        let memories_dir = project_root.join(".codescout").join("memories");
        std::fs::create_dir_all(&memories_dir)?;
        Ok(Self { memories_dir })
    }

    /// Open (or create) a memory store from an explicit directory path.
    /// Used for per-project routing where the caller has already resolved the directory.
    pub fn from_dir(memories_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&memories_dir)?;
        Ok(Self { memories_dir })
    }

    /// Open a store for READING, without creating the directory.
    ///
    /// [`Self::from_dir`] calls `create_dir_all`, which is correct for a write
    /// target and wrong for a read: it makes every read that merely *asks* about
    /// a directory materialise it. `Workspace::memory_dir_for_project`'s doc
    /// comment has named this ("`MemoryStore::from_dir` then creates it on read
    /// as well as on write") since the litter bug archived at
    /// `docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md`.
    ///
    /// Nothing is lost by not creating. [`Self::list`] walks with `walkdir` and
    /// yields nothing for a missing root; [`Self::read`] walks the topic path with
    /// `symlink_metadata` and answers `None` for a missing component. Both already
    /// return the empty answer for an absent
    /// directory — the only thing `create_dir_all` contributed was the directory.
    ///
    /// Infallible by construction, unlike `from_dir`: the creation this omits
    /// was the only fallible step.
    pub fn from_dir_readonly(memories_dir: PathBuf) -> Self {
        Self { memories_dir }
    }

    /// Return the directory this store writes into.
    pub fn dir(&self) -> &Path {
        &self.memories_dir
    }

    /// Open (or create) the private memory store for a project root.
    /// Private memories are gitignored — not shared with teammates.
    /// Automatically adds `.codescout/private-memories/` to `.gitignore`.
    pub fn open_private(project_root: &Path) -> Result<Self> {
        let memories_dir = project_root.join(".codescout").join("private-memories");
        std::fs::create_dir_all(&memories_dir)?;
        Self::ensure_gitignored(project_root, ".codescout/private-memories/")?;
        Ok(Self { memories_dir })
    }

    /// Make sure `entry` is ignored by git, without ever following a link at `.gitignore`.
    ///
    /// The root `.gitignore` is read and rewritten only when it is absent or a regular
    /// file. Anything else (a link — a hostile clone can ship `.gitignore -> ~/.ssh/id_rsa`
    /// — a dangling link, a directory) is left exactly as found: not read through, and not
    /// replaced by a regular file carrying the target's bytes, which is what
    /// [`crate::util::fs::atomic_write`] over an existing link would do. Activation must
    /// not fail on such a project, so [`Self::ignore_from_inside`] puts a `.gitignore` of
    /// `*` inside the store's own directory instead.
    ///
    /// **What is guaranteed.** Either the root `.gitignore` carries `entry`, or the store
    /// directory holds a regular `.gitignore` of exactly `*\n`. When the fallback cannot
    /// establish that (a linked `.codescout` or store directory, an existing
    /// `.gitignore` there with other content or of another type), nothing is changed, a
    /// warning is logged, and the store is NOT known to be ignored.
    fn ensure_gitignored(project_root: &Path, entry: &str) -> Result<()> {
        let gitignore_path = project_root.join(".gitignore");
        let existing = match std::fs::symlink_metadata(&gitignore_path) {
            Ok(m) if m.file_type().is_file() => std::fs::read_to_string(&gitignore_path)?,
            Ok(_) => {
                return Self::ignore_from_inside(project_root, entry, &gitignore_path).map(|_| ())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        if existing.lines().any(|l| l.trim() == entry) {
            return Ok(());
        }
        let mut content = existing;
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        content.push_str(entry);
        content.push('\n');
        crate::util::fs::atomic_write(&gitignore_path, &content)?;
        Ok(())
    }

    /// The fallback of [`Self::ensure_gitignored`] for a `.gitignore` that is not a regular
    /// file: ignore the store from inside it. `entry` is the project-relative directory
    /// whose own `.gitignore` is written; every component of it must be a real directory
    /// (a linked `.codescout` or store directory would send the write elsewhere), and an
    /// existing `.gitignore` there is accepted only if it is a regular file holding exactly
    /// `*\n` — the content `WriteTransaction` requires of its history directory. Anything
    /// else is skipped with a reason, which is logged; nothing existing is overwritten.
    fn ignore_from_inside(project_root: &Path, entry: &str, skipped: &Path) -> Result<SelfIgnore> {
        let outcome = Self::try_ignore_from_inside(project_root, entry)?;
        match &outcome {
            SelfIgnore::Ignored => tracing::warn!(
                "{} is not a regular file; left untouched. `{}` is ignored by a .gitignore inside it instead.",
                skipped.display(),
                entry
            ),
            SelfIgnore::Skipped(reason) => tracing::warn!(
                "{} is not a regular file; left untouched, and `{}` is NOT known to be ignored by git: {}",
                skipped.display(),
                entry,
                reason
            ),
        }
        Ok(outcome)
    }

    fn try_ignore_from_inside(project_root: &Path, entry: &str) -> Result<SelfIgnore> {
        let mut dir = project_root.to_path_buf();
        for component in Path::new(entry.trim_end_matches('/')).components() {
            dir.push(component.as_os_str());
            match std::fs::symlink_metadata(&dir) {
                Ok(m) if m.file_type().is_dir() => {}
                Ok(_) => {
                    return Ok(SelfIgnore::Skipped(format!(
                        "{} is not a real directory",
                        dir.display()
                    )))
                }
                Err(e) => {
                    return Ok(SelfIgnore::Skipped(format!(
                        "cannot inspect {}: {e}",
                        dir.display()
                    )))
                }
            }
        }
        let own = dir.join(".gitignore");
        match std::fs::symlink_metadata(&own) {
            Ok(m) if m.file_type().is_file() => {
                if read_regular_file(&own).ok().as_deref() == Some("*\n") {
                    Ok(SelfIgnore::Ignored)
                } else {
                    Ok(SelfIgnore::Skipped(format!(
                        "{} exists and is not exactly `*`",
                        own.display()
                    )))
                }
            }
            Ok(_) => Ok(SelfIgnore::Skipped(format!(
                "{} exists and is not a regular file",
                own.display()
            ))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut options = std::fs::OpenOptions::new();
                options.write(true).create_new(true);
                match options.open(&own) {
                    Ok(mut file) => {
                        use std::io::Write;
                        file.write_all(b"*\n")?;
                        Ok(SelfIgnore::Ignored)
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(
                        SelfIgnore::Skipped(format!("{} appeared concurrently", own.display())),
                    ),
                    Err(e) => Err(e.into()),
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Replace a topic wholesale, preserving its predecessor in private history.
    pub fn write(&self, topic: &str, content: &str) -> Result<()> {
        write_transaction::WriteTransaction::begin(self, topic)?.write(content)
    }

    /// Preview the shrink predicate without acquiring a write lease.
    /// None includes read failures, so this is not a write authorization.
    /// The guarded write evaluates the predicate from a strict read under its
    /// transaction lease; raw write intentionally omits shrink policy.
    pub fn shrink_check(&self, topic: &str, content: &str) -> Option<ShrinkReport> {
        let existing = self.read(topic).ok().flatten()?;
        crate::util::shrink_guard::check(&existing, content)
    }

    /// Read a memory entry by topic. Returns `None` if not found.
    ///
    /// Refuses, rather than follows, anything that is not a real file below the store
    /// root: a leaf link, a directory at the topic path, or a linked parent directory.
    /// `WriteTransaction` already refuses the same shapes for writes, and reading through
    /// one hands the caller whatever the link points at (a hostile clone can ship
    /// `.codescout/memories/arch.md -> ~/.ssh/id_rsa`). The store ROOT may itself be a
    /// link — that is documented (`docs/conventions/memory-write-history.md`) and only
    /// components below it are inspected. A missing topic, or a missing parent, is still
    /// `Ok(None)`. A component that cannot be inspected at all (EACCES, ENAMETOOLONG) is
    /// a `RecoverableError` naming it; before the walk existed `exists()` hid those as
    /// "not found".
    ///
    /// Reads refuse with [`RecoverableError`] where the write side uses `bail!`
    /// (`isError: true`): a refused read is input the caller can correct, and the
    /// difference is deliberate, not drift.
    ///
    /// The walk is check-then-read, so the final read opens the file with `O_NOFOLLOW`
    /// and checks the descriptor ([`read_regular_file`]); a link swapped in after the
    /// walk is still refused. A swapped-in PARENT directory link is not closed.
    pub fn read(&self, topic: &str) -> Result<Option<String>> {
        let path = self.topic_path(topic);
        let relative = path.strip_prefix(&self.memories_dir)?;
        let mut walked = self.memories_dir.clone();
        let mut components = relative.components().peekable();
        while let Some(component) = components.next() {
            walked.push(component.as_os_str());
            let last = components.peek().is_none();
            match std::fs::symlink_metadata(&walked) {
                Ok(m) if last && m.file_type().is_file() => {
                    return read_regular_file(&path).map(Some);
                }
                Ok(m) if !last && m.file_type().is_dir() => {}
                Ok(_) if last => {
                    return Err(RecoverableError::with_hint(
                        format!("memory target must be a regular file: {}", walked.display()),
                        "A memory topic is read only when it is a regular file; replace the \
                         link or directory with a file, or remove it.",
                    )
                    .into());
                }
                Ok(m) => {
                    let hint = if m.file_type().is_symlink() {
                        "A memory topic is read only through real directories; replace the \
                         link with a directory, or remove it."
                            .to_string()
                    } else {
                        format!(
                            "{} is not a directory; remove or rename it.",
                            walked.display()
                        )
                    };
                    return Err(RecoverableError::with_hint(
                        format!(
                            "memory parent must be a real directory: {}",
                            walked.display()
                        ),
                        hint,
                    )
                    .into());
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => {
                    return Err(RecoverableError::with_hint(
                        format!("cannot inspect memory path {}: {e}", walked.display()),
                        "Check the name's length and the directory's permissions.",
                    )
                    .into());
                }
            }
        }
        Ok(None)
    }

    /// List all memory topics.
    pub fn list(&self) -> Result<Vec<String>> {
        let mut topics = vec![];
        for entry in walkdir::WalkDir::new(&self.memories_dir)
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "md" {
                        if let Ok(rel) = entry.path().strip_prefix(&self.memories_dir) {
                            let topic = rel.with_extension("").to_string_lossy().replace('\\', "/");
                            topics.push(topic);
                        }
                    }
                }
            }
        }
        topics.sort();
        Ok(topics)
    }

    /// Delete a topic under the same lease used by writes.
    pub fn delete(&self, topic: &str) -> Result<()> {
        write_transaction::WriteTransaction::begin_delete(self, topic)?.delete()
    }

    pub(crate) fn topic_path(&self, topic: &str) -> PathBuf {
        let safe = sanitize_topic(topic);
        self.memories_dir.join(safe).with_extension("md")
    }
}

/// What [`MemoryStore::ignore_from_inside`] did. `Skipped` carries the reason that is
/// also logged, so the decision is a value a test can read.
#[derive(Debug, PartialEq, Eq)]
enum SelfIgnore {
    Ignored,
    Skipped(String),
}

/// Read a file's text only if the OPENED descriptor is a regular file.
///
/// On unix the open uses `O_NOFOLLOW` (a link at the final component fails `ELOOP`, which
/// is reported as the same refusal) and `O_NONBLOCK` (so a FIFO swapped in after the walk
/// cannot block the open). The type is then checked on the descriptor itself, not on the
/// path, so nothing swapped in after the open can change what was read.
fn read_regular_file(path: &Path) -> Result<String> {
    use std::io::Read;
    let refuse = || {
        RecoverableError::with_hint(
            format!("memory target must be a regular file: {}", path.display()),
            "A memory topic is read only when it is a regular file; replace the \
             link or directory with a file, or remove it.",
        )
    };
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = match options.open(path) {
        Ok(f) => f,
        #[cfg(unix)]
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => return Err(refuse().into()),
        Err(e) => return Err(e.into()),
    };
    if !file.metadata()?.file_type().is_file() {
        return Err(refuse().into());
    }
    let mut text = String::new();
    (&file).read_to_string(&mut text)?;
    Ok(text)
}

/// Sanitize a memory topic name to prevent directory traversal.
///
/// Uses `Path::components()` to keep only `Normal` segments, discarding
/// `.`, `..`, root prefixes, and embedded separators.
pub(crate) fn sanitize_topic(topic: &str) -> String {
    use std::path::{Component, Path};
    let sanitized: PathBuf = Path::new(topic)
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s),
            _ => None,
        })
        .collect();
    let result = sanitized.to_string_lossy().into_owned();
    if result.is_empty() {
        "_empty".to_string()
    } else {
        result
    }
}

/// Coverage of the semantic memory store against the markdown memories on disk.
///
/// Counterpart to [`crate::retrieval::sync::IndexIntegrity`], for the other collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryIntegrity {
    pub on_disk: usize,
    pub in_store: usize,
    /// On disk with no point in the store — undiscoverable via `recall`.
    pub missing_count: usize,
    pub missing_sample: Vec<String>,
    /// A point whose markdown is gone — a stale row `recall` can still return.
    pub orphan_count: usize,
    pub orphan_sample: Vec<String>,
}

/// How many names to name. Enough to act on, short enough to read.
const MEMORY_INTEGRITY_SAMPLE: usize = 20;

/// The bucket topic-backed memories live in.
///
/// `MemoryStore` topics and this bucket are one namespace: `cross_embed_memory` writes
/// here, and `memory(action="forget")` derives its point id as
/// `point_id_for(project_id, "structured", topic)` (`src/tools/memory/mod.rs:1031`).
///
/// The other buckets have NO disk file by design, which is why coverage must filter
/// rather than compare everything. `memory(action="remember")` upserts straight to the
/// store — no markdown — defaulting to `"unstructured"`, and the preferences surface
/// (`crate::prompts::builders::append_preferences_section`) reads `"preferences"`.
/// Measured on this repo 2026-08-26: 17 `structured`, 1 `preferences` — and comparing
/// unfiltered reported that one preference as a missing-file orphan, a signal no action
/// could ever clear, on a report meant to be checked routinely.
pub const TOPIC_BUCKET: &str = "structured";

/// Compare the memories on disk against the points in the semantic store.
///
/// **Why the store alone cannot answer this.** `cross_embed_memory` is best-effort and
/// non-fatal (`src/tools/memory/mod.rs:871-877`): a failed embed logs a warning, the
/// markdown write proceeds, and no point is ever created. A lost memory therefore leaves
/// *no trace in the store*, so every store-side instrument is blind to it by
/// construction. `reembed_memories_in_place` (`src/migrate/memories.rs:203`) enumerates
/// via `store.list()`, which is why it re-derives vectors for memories that already have
/// points and can never recover the ones that do not. Disk is the only enumeration that
/// sees them — hence `topics` from the writer's own walk ([`MemoryStore::list`]) rather
/// than a re-derivation of eligibility here.
///
/// Measured on this repo 2026-08-26, before any of this existed: 23 on disk, 17 in the
/// `structured` bucket, **8 undiscoverable** (`eval-design` at 31 KB among them), 2 stale
/// points whose markdown was gone.
///
/// **Pass the SHARED topic list only.** `cross_embed_memory` runs under `if !private`, so
/// a private memory correctly has no point; passing `private_memory.list()` here would
/// report every one of them as missing. That is the disk-side twin of the
/// [`TOPIC_BUCKET`] filter on the store side — both exclude a population whose absence is
/// correct, and both, left in, produce a permanently unclearable warning.
///
/// Read-only, on the same grounds as [`crate::retrieval::sync::verify_index_coverage`]: a
/// negative result must never authorise a deletion, so a wrong answer costs a misleading
/// report rather than a destroyed corpus.
pub async fn verify_memory_coverage(
    topics: &[String],
    store: &dyn semantic_store::SemanticMemoryStore,
    project_id: &str,
) -> Result<MemoryIntegrity> {
    use std::collections::BTreeSet;

    let expected: BTreeSet<&str> = topics.iter().map(String::as_str).collect();
    let hits = store
        .list(
            project_id,
            semantic_store::MemoryFilter {
                bucket: Some(TOPIC_BUCKET.to_string()),
                ..Default::default()
            },
        )
        .await?;
    let stored: BTreeSet<&str> = hits.iter().map(|h| h.memory.title.as_str()).collect();

    let missing: Vec<String> = expected
        .difference(&stored)
        .map(|s| (*s).to_string())
        .collect();
    let orphans: Vec<String> = stored
        .difference(&expected)
        .map(|s| (*s).to_string())
        .collect();

    Ok(MemoryIntegrity {
        on_disk: expected.len(),
        in_store: stored.len(),
        missing_count: missing.len(),
        missing_sample: missing.into_iter().take(MEMORY_INTEGRITY_SAMPLE).collect(),
        orphan_count: orphans.len(),
        orphan_sample: orphans.into_iter().take(MEMORY_INTEGRITY_SAMPLE).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_store() -> (tempfile::TempDir, MemoryStore) {
        let dir = tempdir().unwrap();
        let store = MemoryStore::open(dir.path()).unwrap();
        (dir, store)
    }

    #[test]
    fn open_private_creates_private_memories_dir() {
        let dir = tempdir().unwrap();
        let _store = MemoryStore::open_private(dir.path()).unwrap();
        assert!(dir.path().join(".codescout/private-memories").is_dir());
    }

    #[test]
    fn open_private_adds_to_gitignore() {
        let dir = tempdir().unwrap();
        MemoryStore::open_private(dir.path()).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains(".codescout/private-memories/"));
    }

    #[test]
    fn open_private_does_not_duplicate_gitignore_entry() {
        let dir = tempdir().unwrap();
        MemoryStore::open_private(dir.path()).unwrap();
        MemoryStore::open_private(dir.path()).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        let count = content
            .lines()
            .filter(|l| l.trim() == ".codescout/private-memories/")
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn open_private_appends_to_existing_gitignore() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        MemoryStore::open_private(dir.path()).unwrap();
        let content = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(content.contains("target/\n"));
        assert!(content.contains(".codescout/private-memories/"));
    }
    /// A regular `.gitignore` whose last line has no newline keeps that line and gains
    /// ours on a line of its own — the normal path the leaf-link guard must not disturb.
    #[test]
    fn open_private_appends_after_a_gitignore_with_no_trailing_newline() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/").unwrap();
        MemoryStore::open_private(dir.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
            "target/\n.codescout/private-memories/\n"
        );
    }

    /// F12. A hostile repository ships `.gitignore` as a link to a secret. Activation reads
    /// that file to decide whether to append the private-memories entry, and the append
    /// used to be written back as a REGULAR file — the target's bytes copied into a
    /// tracked project file. The link must survive, the target must be untouched, and no
    /// regular file in the project may carry the target's bytes.
    #[cfg(unix)]
    #[test]
    fn open_private_never_copies_a_gitignore_symlink_target_into_the_project() {
        let sandbox = tempdir().unwrap();
        let secret = sandbox.path().join("home/.ssh/id_rsa");
        std::fs::create_dir_all(secret.parent().unwrap()).unwrap();
        std::fs::write(&secret, "SECRET-BYTES\n").unwrap();
        let root = sandbox.path().join("project");
        std::fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&secret, root.join(".gitignore")).unwrap();

        MemoryStore::open_private(&root).expect("a link in .gitignore must not refuse activation");

        let meta = std::fs::symlink_metadata(root.join(".gitignore")).unwrap();
        assert!(
            meta.file_type().is_symlink(),
            ".gitignore must still be the link, not a regular file holding the target's bytes"
        );
        assert_eq!(
            std::fs::read_to_string(&secret).unwrap(),
            "SECRET-BYTES\n",
            "the link target must be byte-identical afterwards"
        );
        for entry in walkdir::WalkDir::new(&root).into_iter().flatten() {
            if entry.file_type().is_file() {
                let body = std::fs::read_to_string(entry.path()).unwrap_or_default();
                assert!(
                    !body.contains("SECRET-BYTES"),
                    "{} carries the link target's bytes",
                    entry.path().display()
                );
            }
        }
    }

    /// Declining to touch the root `.gitignore` must not leave the private store
    /// committable: the store then ignores itself, with a file that is written
    /// exclusively and so cannot be pointed elsewhere.
    #[cfg(unix)]
    #[test]
    fn open_private_self_ignores_when_the_root_gitignore_is_a_link() {
        let sandbox = tempdir().unwrap();
        let target = sandbox.path().join("shared-gitignore");
        std::fs::write(&target, "target/\n").unwrap();
        let root = sandbox.path().join("project");
        std::fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&target, root.join(".gitignore")).unwrap();

        MemoryStore::open_private(&root).unwrap();

        assert_eq!(
            std::fs::read_to_string(root.join(".codescout/private-memories/.gitignore")).unwrap(),
            "*\n"
        );
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "target/\n");
    }

    /// A dangling link is not "missing": creating the file would replace the link.
    #[cfg(unix)]
    #[test]
    fn open_private_leaves_a_dangling_gitignore_link_alone() {
        let sandbox = tempdir().unwrap();
        let root = sandbox.path().join("project");
        std::fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(sandbox.path().join("nowhere"), root.join(".gitignore"))
            .unwrap();

        MemoryStore::open_private(&root).unwrap();

        assert!(std::fs::symlink_metadata(root.join(".gitignore"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(!sandbox.path().join("nowhere").exists());
    }

    /// A directory at `.gitignore` made activation fail on the read; it must degrade the
    /// same way a link does.
    #[test]
    fn open_private_survives_a_gitignore_that_is_a_directory() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".gitignore")).unwrap();

        MemoryStore::open_private(dir.path()).unwrap();

        assert!(dir.path().join(".gitignore").is_dir());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".codescout/private-memories/.gitignore"))
                .unwrap(),
            "*\n"
        );
    }

    /// The fallback file is itself never written through a link planted at its name.
    #[cfg(unix)]
    #[test]
    fn the_self_ignore_fallback_is_never_written_through_a_link() {
        let sandbox = tempdir().unwrap();
        let victim = sandbox.path().join("victim");
        std::fs::write(&victim, "KEEP\n").unwrap();
        let root = sandbox.path().join("project");
        let private = root.join(".codescout/private-memories");
        std::fs::create_dir_all(&private).unwrap();
        std::fs::create_dir(root.join(".gitignore")).unwrap();
        std::os::unix::fs::symlink(&victim, private.join(".gitignore")).unwrap();

        MemoryStore::open_private(&root).unwrap();

        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "KEEP\n");
        assert!(std::fs::symlink_metadata(private.join(".gitignore"))
            .unwrap()
            .file_type()
            .is_symlink());
    }

    /// Review MEDIUM-2. The fallback joins `.codescout/private-memories` onto the project
    /// root; if that directory is itself a link, `open_private` followed it and the
    /// fallback created `<outside>/.gitignore`. Neither component may be a link.
    #[cfg(unix)]
    #[test]
    fn the_self_ignore_fallback_never_writes_through_a_linked_private_dir() {
        let sandbox = tempdir().unwrap();
        let outside = sandbox.path().join("outside-dir");
        std::fs::create_dir_all(&outside).unwrap();
        let root = sandbox.path().join("project");
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::create_dir(root.join(".gitignore")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join(".codescout/private-memories")).unwrap();

        MemoryStore::open_private(&root).expect("activation must not fail");

        assert!(
            !outside.join(".gitignore").exists(),
            "the fallback wrote through the linked private-memories directory"
        );
    }

    /// The same, one level up: `.codescout` itself a link to a directory elsewhere.
    #[cfg(unix)]
    #[test]
    fn the_self_ignore_fallback_never_writes_through_a_linked_codescout_dir() {
        let sandbox = tempdir().unwrap();
        let outside = sandbox.path().join("outside-codescout");
        std::fs::create_dir_all(&outside).unwrap();
        let root = sandbox.path().join("project");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir(root.join(".gitignore")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join(".codescout")).unwrap();

        MemoryStore::open_private(&root).expect("activation must not fail");

        assert!(
            !outside.join("private-memories/.gitignore").exists(),
            "the fallback wrote through the linked .codescout directory"
        );
    }

    /// Review LOW-1. The fallback used to accept ANY existing `private-memories/.gitignore`,
    /// so a shipped `# ignores nothing` file left the private store committable while the
    /// docs said it ignores itself. Only the exact `*\n` that `WriteTransaction` expects
    /// counts; anything else is skipped with a reason (and logged), never overwritten.
    #[test]
    fn the_self_ignore_fallback_accepts_only_an_existing_star_file() {
        let sandbox = tempdir().unwrap();
        let root = sandbox.path();
        let private = root.join(".codescout/private-memories");
        std::fs::create_dir_all(&private).unwrap();
        let skipped = root.join(".gitignore");
        let entry = ".codescout/private-memories/";

        std::fs::write(private.join(".gitignore"), "# ignores nothing\n").unwrap();
        let outcome = MemoryStore::ignore_from_inside(root, entry, &skipped).unwrap();
        assert!(matches!(outcome, SelfIgnore::Skipped(_)), "got {outcome:?}");
        assert_eq!(
            std::fs::read_to_string(private.join(".gitignore")).unwrap(),
            "# ignores nothing\n",
            "a user's file must not be overwritten"
        );

        std::fs::write(private.join(".gitignore"), "*\n").unwrap();
        assert_eq!(
            MemoryStore::ignore_from_inside(root, entry, &skipped).unwrap(),
            SelfIgnore::Ignored
        );
    }

    #[test]
    fn the_self_ignore_fallback_writes_star_when_the_file_is_absent() {
        let sandbox = tempdir().unwrap();
        let root = sandbox.path();
        std::fs::create_dir_all(root.join(".codescout/private-memories")).unwrap();

        let outcome = MemoryStore::ignore_from_inside(
            root,
            ".codescout/private-memories/",
            &root.join(".gitignore"),
        )
        .unwrap();

        assert_eq!(outcome, SelfIgnore::Ignored);
        assert_eq!(
            std::fs::read_to_string(root.join(".codescout/private-memories/.gitignore")).unwrap(),
            "*\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_self_ignore_fallback_skips_a_linked_private_dir_with_a_reason() {
        let sandbox = tempdir().unwrap();
        let outside = sandbox.path().join("outside-dir");
        std::fs::create_dir_all(&outside).unwrap();
        let root = sandbox.path().join("project");
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join(".codescout/private-memories")).unwrap();

        let outcome = MemoryStore::ignore_from_inside(
            &root,
            ".codescout/private-memories/",
            &root.join(".gitignore"),
        )
        .unwrap();

        assert!(matches!(outcome, SelfIgnore::Skipped(_)), "got {outcome:?}");
    }

    #[test]
    fn write_and_read_roundtrip() {
        let (_dir, store) = make_store();
        store.write("my-topic", "hello memory").unwrap();
        assert_eq!(
            store.read("my-topic").unwrap(),
            Some("hello memory".to_string())
        );
    }

    #[test]
    fn read_missing_returns_none() {
        let (_dir, store) = make_store();
        assert_eq!(store.read("does-not-exist").unwrap(), None);
    }

    fn assert_refused_as_non_regular(err: &anyhow::Error) {
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .unwrap_or_else(|| panic!("must be a RecoverableError, got: {err:#}"));
        assert!(
            rec.to_string()
                .contains("memory target must be a regular file"),
            "got: {rec}"
        );
    }

    #[test]
    fn read_of_a_missing_nested_topic_is_none() {
        let (_dir, store) = make_store();
        assert_eq!(store.read("no/such/deep-topic").unwrap(), None);
    }

    /// F13. `arch.md -> ~/.ssh/id_rsa`: `read` followed the link and returned the bytes.
    #[cfg(unix)]
    #[test]
    fn read_refuses_a_leaf_symlink_to_a_file_outside_the_store() {
        let (dir, store) = make_store();
        let secret = dir.path().join("home/.ssh/id_rsa");
        std::fs::create_dir_all(secret.parent().unwrap()).unwrap();
        std::fs::write(&secret, "PRIVATE").unwrap();
        std::os::unix::fs::symlink(&secret, store.dir().join("arch.md")).unwrap();

        let err = store.read("arch").expect_err("a leaf link must be refused");

        assert_refused_as_non_regular(&err);
        assert!(!format!("{err:#}").contains("PRIVATE"));
    }

    /// Consistency, not only exfiltration: `WriteTransaction` refuses EVERY leaf link, so a
    /// link to a sibling inside the store is refused for reading too. No shipped flow
    /// creates one (measured 2026-10-18: 0 tracked symlinks under `.codescout/`).
    #[cfg(unix)]
    #[test]
    fn read_refuses_a_leaf_symlink_to_a_file_inside_the_store() {
        let (_dir, store) = make_store();
        store.write("real", "inside").unwrap();
        std::os::unix::fs::symlink(store.dir().join("real.md"), store.dir().join("alias.md"))
            .unwrap();

        assert_refused_as_non_regular(&store.read("alias").expect_err("must be refused"));
        assert_eq!(store.read("real").unwrap().as_deref(), Some("inside"));
    }

    /// A dangling link is not "no such topic": `WriteTransaction` refuses it too.
    #[cfg(unix)]
    #[test]
    fn read_refuses_a_dangling_leaf_symlink() {
        let (dir, store) = make_store();
        std::os::unix::fs::symlink(dir.path().join("nowhere"), store.dir().join("ghost.md"))
            .unwrap();

        assert_refused_as_non_regular(&store.read("ghost").expect_err("must be refused"));
    }

    #[test]
    fn read_refuses_a_directory_at_the_topic_path() {
        let (_dir, store) = make_store();
        std::fs::create_dir(store.dir().join("dir.md")).unwrap();

        assert_refused_as_non_regular(&store.read("dir").expect_err("must be refused"));
    }

    /// The parent-directory twin: `nested -> <outside dir>` made `read("nested/topic")`
    /// reach `<outside>/topic.md`. `safe_topic_parent` refuses this for writes.
    #[cfg(unix)]
    #[test]
    fn read_refuses_a_topic_below_a_symlinked_directory() {
        let (dir, store) = make_store();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("topic.md"), "PRIVATE").unwrap();
        std::os::unix::fs::symlink(&outside, store.dir().join("nested")).unwrap();

        let err = store
            .read("nested/topic")
            .expect_err("a linked parent must be refused");

        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .unwrap_or_else(|| panic!("must be a RecoverableError, got: {err:#}"));
        assert!(
            rec.to_string()
                .contains("memory parent must be a real directory"),
            "got: {rec}"
        );
    }

    /// A plain file where a parent directory belongs is "not a directory", not a link to
    /// replace: the hint must not tell the user to "replace the link".
    #[test]
    fn read_below_a_plain_file_says_it_is_not_a_directory() {
        let (_dir, store) = make_store();
        std::fs::write(store.dir().join("nested"), "i am a file").unwrap();

        let err = store.read("nested/topic").expect_err("must be refused");

        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .unwrap_or_else(|| panic!("must be a RecoverableError, got: {err:#}"));
        let text = rec.to_string();
        assert!(
            text.contains("memory parent must be a real directory"),
            "got: {text}"
        );
        assert!(text.contains("is not a directory"), "got: {text}");
        assert!(!text.contains("replace the link"), "got: {text}");
    }

    /// Review LOW-2. The base answered "topic not found" for a component it could not
    /// stat (`exists()` is false on any error); the new walk returned the raw io error as
    /// a fatal `isError: true`. It is a `RecoverableError` naming the component.
    #[test]
    fn read_of_an_over_long_name_is_a_recoverable_error_naming_the_component() {
        let (_dir, store) = make_store();
        let topic = "x".repeat(300);

        let err = store
            .read(&topic)
            .expect_err("ENAMETOOLONG is not 'not found'");

        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .unwrap_or_else(|| panic!("must be a RecoverableError, got: {err:#}"));
        assert!(
            rec.to_string().contains(&"x".repeat(300)),
            "the component must be named: {rec}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn read_below_an_unsearchable_directory_is_a_recoverable_error_naming_it() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, store) = make_store();
        let nested = store.dir().join("locked");
        std::fs::create_dir(&nested).unwrap();
        std::fs::write(nested.join("topic.md"), "x").unwrap();
        std::fs::set_permissions(&nested, std::fs::Permissions::from_mode(0o000)).unwrap();
        // root ignores mode bits, so the premise would not hold.
        let probe = std::fs::symlink_metadata(nested.join("topic.md"));
        let can_test = probe.is_err();

        let result = store.read("locked/topic");
        std::fs::set_permissions(&nested, std::fs::Permissions::from_mode(0o755)).unwrap();
        if !can_test {
            return;
        }

        let err = result.expect_err("EACCES must surface");
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .unwrap_or_else(|| panic!("must be a RecoverableError, got: {err:#}"));
        assert!(rec.to_string().contains("topic.md"), "got: {rec}");
    }

    /// Review LOW-4. The path walk is check-then-read; the read itself must not follow a
    /// final-component link either, and must check the DESCRIPTOR it opened.
    #[cfg(unix)]
    #[test]
    fn the_final_read_refuses_a_link_and_a_directory_even_without_the_walk() {
        let dir = tempdir().unwrap();
        let secret = dir.path().join("secret");
        std::fs::write(&secret, "SECRET-BYTES").unwrap();
        let link = dir.path().join("link.md");
        std::os::unix::fs::symlink(&secret, &link).unwrap();
        let folder = dir.path().join("folder.md");
        std::fs::create_dir(&folder).unwrap();
        let file = dir.path().join("file.md");
        std::fs::write(&file, "body").unwrap();

        assert_eq!(read_regular_file(&file).unwrap(), "body");
        for refused in [&link, &folder] {
            let err = read_regular_file(refused).expect_err("must be refused");
            assert_refused_as_non_regular(&err);
            assert!(!format!("{err:#}").contains("SECRET-BYTES"));
        }
    }

    /// The store root itself may be a link (documented in
    /// `docs/conventions/memory-write-history.md`); only components BELOW it are checked.
    #[cfg(unix)]
    #[test]
    fn read_through_a_symlinked_store_root_still_works() {
        let dir = tempdir().unwrap();
        let actual = dir.path().join("actual");
        std::fs::create_dir_all(&actual).unwrap();
        std::fs::write(actual.join("topic.md"), "kept").unwrap();
        let configured = dir.path().join("configured");
        std::os::unix::fs::symlink(&actual, &configured).unwrap();

        let store = MemoryStore::from_dir_readonly(configured);

        assert_eq!(store.read("topic").unwrap().as_deref(), Some("kept"));
    }

    /// `list` walks without following links, so a link is simply not a topic. Pinned so a
    /// switch to `follow_links(true)` cannot silently re-open the read side.
    #[cfg(unix)]
    #[test]
    fn list_does_not_report_or_enter_symlinks() {
        let (dir, store) = make_store();
        store.write("real", "x").unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("hidden.md"), "PRIVATE").unwrap();
        std::os::unix::fs::symlink(outside.join("hidden.md"), store.dir().join("leaf.md")).unwrap();
        std::os::unix::fs::symlink(&outside, store.dir().join("nested")).unwrap();

        assert_eq!(store.list().unwrap(), vec!["real".to_string()]);
    }

    #[test]
    fn list_empty_store() {
        let (_dir, store) = make_store();
        assert_eq!(store.list().unwrap(), Vec::<String>::new());
    }

    #[test]
    fn list_after_writes_is_sorted() {
        let (_dir, store) = make_store();
        store.write("c-topic", "c").unwrap();
        store.write("a-topic", "a").unwrap();
        store.write("b-topic", "b").unwrap();
        let list = store.list().unwrap();
        assert_eq!(list, vec!["a-topic", "b-topic", "c-topic"]);
    }

    #[test]
    fn delete_removes_entry() {
        let (_dir, store) = make_store();
        store.write("to-delete", "content").unwrap();
        store.delete("to-delete").unwrap();
        assert_eq!(store.read("to-delete").unwrap(), None);
        assert!(!store.list().unwrap().contains(&"to-delete".to_string()));
    }

    #[test]
    fn delete_nonexistent_is_ok() {
        let (_dir, store) = make_store();
        assert!(store.delete("ghost").is_ok());
    }

    #[test]
    fn nested_topic_roundtrip() {
        let (_dir, store) = make_store();
        store
            .write("debugging/async-patterns", "avoid blocking")
            .unwrap();
        assert_eq!(
            store.read("debugging/async-patterns").unwrap(),
            Some("avoid blocking".to_string())
        );
        assert!(store
            .list()
            .unwrap()
            .contains(&"debugging/async-patterns".to_string()));
    }

    #[test]
    fn overwrite_replaces_content() {
        let (_dir, store) = make_store();
        store.write("key", "v1").unwrap();
        store.write("key", "v2").unwrap();
        assert_eq!(store.read("key").unwrap(), Some("v2".to_string()));
    }

    // ── shrink guard (CM-6) ─────────────────────────────────────────────
    //
    // `write` replaces wholesale by design — `overwrite_replaces_content`
    // above pins that, and `tools/onboarding.rs` relies on it. The guard is
    // therefore a separate, non-mutating check the TOOL layer consults,
    // mirroring how the artifact guard lives in `librarian/tools/update.rs`
    // rather than in the catalog write primitive.
    //
    // Repro that motivated it: writing 2 sections to a 17-section memory
    // destroyed 15 of them and returned `{"status":"ok"}`.
    // See docs/issues/archive/2026-08-28-memory-write-has-no-shrink-guard.md.

    #[test]
    fn shrink_check_flags_a_destructive_overwrite() {
        // The exact byte counts measured in the reproduction: a 751-byte,
        // 10-section memory replaced by a 112-byte, 1-section write.
        let (_dir, store) = make_store();
        store.write("victim", &"X".repeat(751)).unwrap();

        let report = store
            .shrink_check("victim", &"Y".repeat(112))
            .expect("the measured 751 -> 112 overwrite must be flagged");
        assert_eq!(report.old_bytes, 751);
        assert_eq!(report.new_bytes, 112);
        // 100 - (112*100/751) under integer division. Truncating toward zero
        // makes this read one point WORSE than the true 85.1%, which is the
        // safe direction for a warning and matches the artifact guard's
        // formula byte for byte.
        assert_eq!(
            report.byte_pct, 86,
            "the percentage is what the caller shows"
        );
        // Single-line content either side, so the line arm is structurally
        // unable to fire — this case must be attributed to bytes alone.
        assert_eq!(
            report.dimension,
            crate::util::shrink_guard::ShrinkDimension::Bytes
        );
    }

    #[test]
    fn shrink_check_is_silent_when_the_write_grows() {
        let (_dir, store) = make_store();
        store.write("victim", &"X".repeat(400)).unwrap();
        assert!(store.shrink_check("victim", &"Y".repeat(900)).is_none());
    }

    #[test]
    fn shrink_check_is_silent_for_a_new_topic() {
        let (_dir, store) = make_store();
        assert!(
            store.shrink_check("never-written", "x").is_none(),
            "a first write destroys nothing"
        );
    }

    #[test]
    fn shrink_check_is_silent_below_the_byte_floor() {
        let (_dir, store) = make_store();
        // Under the floor a ratio is meaningless — a 3-byte stub replaced by a
        // 1-byte one is a 66% "loss" and nobody cares.
        store
            .write("stub", &"X".repeat(SHRINK_GUARD_MIN_BYTES - 1))
            .unwrap();
        assert!(store.shrink_check("stub", "y").is_none());
    }

    /// The predicate is `new * 2 < old`, so removing EXACTLY half is allowed.
    /// Pinned because the obvious `<=` spelling would flip this, and a
    /// boundary that moves silently is how a guard starts refusing good writes.
    #[test]
    fn shrink_check_permits_removing_exactly_half() {
        let (_dir, store) = make_store();
        store.write("half", &"X".repeat(800)).unwrap();

        assert!(
            store.shrink_check("half", &"Y".repeat(400)).is_none(),
            "exactly half must pass"
        );
        assert!(
            store.shrink_check("half", &"Y".repeat(399)).is_some(),
            "one byte past half must fail — proves the boundary is live"
        );
    }

    #[test]
    fn dotdot_in_topic_is_sanitized() {
        let (_dir, store) = make_store();
        // Should not escape the memories directory
        store.write("../escape", "evil").unwrap();
        // Reading with the same (sanitized) key works
        let result = store.read("../escape").unwrap();
        assert_eq!(result, Some("evil".to_string()));
    }

    #[test]
    fn absolute_path_topic_stays_inside_memories_dir() {
        let (_dir, store) = make_store();
        // An absolute path in topic should NOT escape the memories directory.
        // PathBuf::join with an absolute path replaces the base — this tests that
        // topic_path prevents that.
        let evil_topic = "/etc/shadow";
        let resolved = store.topic_path(evil_topic);
        assert!(
            resolved.starts_with(&store.memories_dir),
            "absolute path topic escaped memories dir: {:?}",
            resolved
        );
    }

    #[test]
    fn topic_with_null_byte_is_handled() {
        let (_dir, store) = make_store();
        // Null bytes in filenames can cause truncation in C-based syscalls.
        let result = store.write("safe\0evil", "content");
        // Should either succeed safely or return an error — not panic.
        // The important thing is the file (if created) stays inside memories_dir.
        if result.is_ok() {
            let path = store.topic_path("safe\0evil");
            assert!(path.starts_with(&store.memories_dir));
        }
    }

    #[test]
    fn topic_with_backslash_traversal_stays_inside() {
        let (_dir, store) = make_store();
        // Windows-style path traversal attempt
        let resolved = store.topic_path("..\\..\\etc\\passwd");
        assert!(
            resolved.starts_with(&store.memories_dir),
            "backslash traversal escaped memories dir: {:?}",
            resolved
        );
    }

    #[test]
    fn empty_topic_does_not_panic() {
        let (_dir, store) = make_store();
        // Empty topic should not panic
        let resolved = store.topic_path("");
        assert!(resolved.starts_with(&store.memories_dir));
    }

    #[test]
    fn deeply_nested_topic_works() {
        let (_dir, store) = make_store();
        store.write("a/b/c/d/e/deep-topic", "deep content").unwrap();
        assert_eq!(
            store.read("a/b/c/d/e/deep-topic").unwrap(),
            Some("deep content".to_string())
        );
    }

    #[test]
    fn topic_with_special_chars() {
        let (_dir, store) = make_store();
        // Topics with special characters should work or fail gracefully
        for topic in &["hello world", "a&b", "test=value", "name@domain"] {
            let result = store.write(topic, "content");
            if result.is_ok() {
                assert_eq!(store.read(topic).unwrap(), Some("content".to_string()));
            }
            // Either works or returns error — no panic
        }
    }

    #[test]
    fn topic_path_blocks_dot_slash_traversal() {
        let (_dir, store) = make_store();
        let path = store.topic_path("a/./b/../../../etc/passwd");
        assert!(
            path.starts_with(&store.memories_dir),
            "path {:?} must be inside {:?}",
            path,
            store.memories_dir,
        );
    }

    #[test]
    fn topic_path_blocks_single_dot() {
        let (_dir, store) = make_store();
        let path = store.topic_path(".");
        assert!(
            path.starts_with(&store.memories_dir),
            "path {:?} must be inside {:?}",
            path,
            store.memories_dir,
        );
        // Must be a file path, not the directory itself
        assert_ne!(path, store.memories_dir);
    }

    #[test]
    fn dashboard_topic_is_sanitized() {
        // The dashboard handler extracts `topic` from the URL path and passes it
        // to MemoryStore::read/write. sanitize_topic is used inside topic_path,
        // so the dashboard is already protected. This test confirms it (C-10).
        let (_dir, store) = make_store();
        let path = store.topic_path("../../etc/passwd");
        assert!(path.starts_with(&store.memories_dir));
    }

    // --- verify_memory_coverage ---

    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::SemanticMemoryStore;
    use crate::retrieval::memory_payload::SemanticMemory;

    async fn store_with(project: &str, titles: &[&str]) -> InMemorySemanticMemoryStore {
        let store = InMemorySemanticMemoryStore::default();
        for t in titles {
            let m = SemanticMemory {
                project_id: project.to_string(),
                bucket: "structured".to_string(),
                title: (*t).to_string(),
                content: format!("content of {t}"),
                anchors: vec![],
                created_at: "2026-08-26T00:00:00Z".to_string(),
                updated_at: "2026-08-26T00:00:00Z".to_string(),
            };
            store.upsert(&m, &[0.1, 0.2, 0.3]).await.unwrap();
        }
        store
    }

    /// The discrimination test, and the reason it is first: a memory present on BOTH
    /// sides must land in neither list.
    ///
    /// A mutation that dropped either operand of the diff — reporting `expected` as
    /// missing, or `stored` as orphaned — still satisfies a test that only asserts
    /// `missing_count == 1`. This one fails for both mutations, because the shared topic
    /// would appear in a list it must never appear in.
    #[tokio::test]
    async fn coverage_separates_missing_orphan_and_present() {
        let store = store_with("p", &["shared", "orphaned"]).await;
        let topics = vec!["shared".to_string(), "lost".to_string()];

        let m = verify_memory_coverage(&topics, &store, "p").await.unwrap();

        assert_eq!(m.on_disk, 2);
        assert_eq!(m.in_store, 2);
        assert_eq!(m.missing_sample, vec!["lost"], "on disk, no point");
        assert_eq!(m.orphan_sample, vec!["orphaned"], "point, no file");
        assert!(
            !m.missing_sample.contains(&"shared".to_string())
                && !m.orphan_sample.contains(&"shared".to_string()),
            "a topic present on both sides must appear in NEITHER list: {m:?}"
        );
    }

    /// Nested topics are slash-joined by [`MemoryStore::list`] and stored verbatim as the
    /// title, so they must match rather than read as missing. `infra/friction-measurement`
    /// is a real one on this repo.
    #[tokio::test]
    async fn coverage_matches_nested_topics() {
        let store = store_with("p", &["infra/friction-measurement"]).await;
        let topics = vec!["infra/friction-measurement".to_string()];

        let m = verify_memory_coverage(&topics, &store, "p").await.unwrap();

        assert_eq!(m.missing_count, 0, "nested topic must match: {m:?}");
        assert_eq!(m.orphan_count, 0);
    }

    /// Another project's points must not count as coverage for this one. The tempdir
    /// pollution measured 2026-08-26 (1959 fixture points under 1909 `.tmp*` project ids)
    /// makes this the realistic failure: a cross-project leak would report a broken
    /// index as complete.
    #[tokio::test]
    async fn coverage_ignores_other_projects_points() {
        let store = store_with("other-project", &["gotchas"]).await;
        let topics = vec!["gotchas".to_string()];

        let m = verify_memory_coverage(&topics, &store, "p").await.unwrap();

        assert_eq!(m.in_store, 0, "must not see another project's points");
        assert_eq!(m.missing_count, 1, "so the topic IS missing here: {m:?}");
    }

    #[tokio::test]
    async fn coverage_clean_when_both_sides_agree() {
        let store = store_with("p", &["a", "b"]).await;
        let topics = vec!["a".to_string(), "b".to_string()];

        let m = verify_memory_coverage(&topics, &store, "p").await.unwrap();

        assert_eq!((m.missing_count, m.orphan_count), (0, 0), "{m:?}");
        assert!(m.missing_sample.is_empty() && m.orphan_sample.is_empty());
    }

    /// The sample is capped but the COUNT is not — a report that silently truncated its
    /// own total would understate the damage while looking precise.
    #[tokio::test]
    async fn coverage_caps_sample_but_not_count() {
        let store = store_with("p", &[]).await;
        let topics: Vec<String> = (0..35).map(|i| format!("topic-{i:02}")).collect();

        let m = verify_memory_coverage(&topics, &store, "p").await.unwrap();

        assert_eq!(m.missing_count, 35, "count must be the true total");
        assert_eq!(m.missing_sample.len(), MEMORY_INTEGRITY_SAMPLE);
    }

    /// A store-only memory in another bucket is NOT an orphan — it has no disk file by
    /// design.
    ///
    /// `memory(action="remember")` upserts straight to the store with no markdown
    /// (`src/tools/memory/mod.rs:1044`), and the preferences surface reads its own bucket.
    /// Comparing unfiltered reports every one of them as a missing-file orphan, forever,
    /// with no action that could clear it — and a report that always complains is one
    /// nobody reads. Caught on the LIVE read of this tool, not by the first five tests
    /// here: a real `preferences` memory was reported as an orphan.
    #[tokio::test]
    async fn coverage_ignores_points_outside_the_topic_bucket() {
        let store = InMemorySemanticMemoryStore::default();
        for (bucket, title) in [
            (TOPIC_BUCKET, "gotchas"),
            ("preferences", "Auto mode kept on"),
            ("unstructured", "some remembered note"),
        ] {
            let m = SemanticMemory {
                project_id: "p".to_string(),
                bucket: bucket.to_string(),
                title: title.to_string(),
                content: "c".to_string(),
                anchors: vec![],
                created_at: "2026-08-26T00:00:00Z".to_string(),
                updated_at: "2026-08-26T00:00:00Z".to_string(),
            };
            store.upsert(&m, &[0.1, 0.2, 0.3]).await.unwrap();
        }

        let m = verify_memory_coverage(&["gotchas".to_string()], &store, "p")
            .await
            .unwrap();

        assert_eq!(m.in_store, 1, "only the topic bucket counts: {m:?}");
        assert_eq!(
            m.orphan_count, 0,
            "a store-only memory in another bucket is not an orphan: {m:?}"
        );
        assert_eq!(m.missing_count, 0);
    }
}
