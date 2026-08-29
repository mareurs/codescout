//! Markdown-based persistent memory store (mirrors Serena's memory system).
//!
//! Memories are stored as `.md` files in `.codescout/memories/`.
//! They are organized hierarchically via path-like topics:
//! e.g. "debugging/async-patterns" → `.codescout/memories/debugging/async-patterns.md`

pub mod anchors;
pub mod classify;
pub mod filter;
pub mod hash;
pub mod semantic_store;
pub mod sqlite_semantic_store;

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
    /// yields nothing for a missing root; [`Self::read`] gates on
    /// `path.exists()`. Both already return the empty answer for an absent
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

    fn ensure_gitignored(project_root: &Path, entry: &str) -> Result<()> {
        let gitignore_path = project_root.join(".gitignore");
        let existing = if gitignore_path.exists() {
            std::fs::read_to_string(&gitignore_path)?
        } else {
            String::new()
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

    /// Write or overwrite a memory entry.
    pub fn write(&self, topic: &str, content: &str) -> Result<()> {
        let path = self.topic_path(topic);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::util::fs::atomic_write(&path, content)?;
        Ok(())
    }

    /// Would writing `content` over `topic` destroy more than half of what is
    /// already there, by bytes **or** by lines? Returns the numbers to show the
    /// caller, or `None` when the write is safe.
    ///
    /// **Non-mutating, and deliberately not wired into [`Self::write`].**
    /// Wholesale replacement is `write`'s specified behaviour — see the
    /// `overwrite_replaces_content` test — and `tools/onboarding.rs` depends
    /// on it to regenerate `onboarding` and `language-patterns`. Policy lives
    /// with the caller that has a user to warn and a `force` flag to offer,
    /// exactly as the artifact body-shrink guard sits in
    /// `librarian/tools/update.rs` rather than in the catalog write primitive.
    ///
    /// A new topic, or an existing one below [`SHRINK_GUARD_MIN_BYTES`],
    /// always returns `None`: the first destroys nothing, and a ratio over a
    /// handful of bytes is noise.
    pub fn shrink_check(&self, topic: &str, content: &str) -> Option<ShrinkReport> {
        // A read error is not a shrink. Refusing a write because the existing
        // entry could not be read would turn an unrelated IO fault into a
        // blocked save, so an unreadable entry declines to object.
        let existing = self.read(topic).ok().flatten()?;
        crate::util::shrink_guard::check(&existing, content)
    }

    /// Read a memory entry by topic. Returns `None` if not found.
    pub fn read(&self, topic: &str) -> Result<Option<String>> {
        let path = self.topic_path(topic);
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
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

    /// Delete a memory entry.
    pub fn delete(&self, topic: &str) -> Result<()> {
        let path = self.topic_path(topic);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    pub(crate) fn topic_path(&self, topic: &str) -> PathBuf {
        let safe = sanitize_topic(topic);
        self.memories_dir.join(safe).with_extension("md")
    }
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
