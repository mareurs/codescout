//! Markdown-based persistent memory store (mirrors Serena's memory system).
//!
//! Memories are stored as `.md` files in `.code-explorer/memories/`.
//! They are organized hierarchically via path-like topics:
//! e.g. "debugging/async-patterns" → `.code-explorer/memories/debugging/async-patterns.md`

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Per-project memory store.
#[derive(Debug, Clone)]
pub struct MemoryStore {
    memories_dir: PathBuf,
}

impl MemoryStore {
    /// Open (or create) the memory store for a project root.
    pub fn open(project_root: &Path) -> Result<Self> {
        let memories_dir = project_root.join(".code-explorer").join("memories");
        std::fs::create_dir_all(&memories_dir)?;
        Ok(Self { memories_dir })
    }

    /// Write or overwrite a memory entry.
    pub fn write(&self, topic: &str, content: &str) -> Result<()> {
        let path = self.topic_path(topic);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
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

    fn topic_path(&self, topic: &str) -> PathBuf {
        // Sanitize: replace ".." components to prevent directory traversal.
        let safe = topic.replace("..", "__");
        self.memories_dir.join(safe).with_extension("md")
    }
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

    #[test]
    fn dotdot_in_topic_is_sanitized() {
        let (_dir, store) = make_store();
        // Should not escape the memories directory
        store.write("../escape", "evil").unwrap();
        // Reading with the same (sanitized) key works
        let result = store.read("../escape").unwrap();
        assert_eq!(result, Some("evil".to_string()));
    }
}
