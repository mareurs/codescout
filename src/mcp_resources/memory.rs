use super::{ResourceBytes, ResourceDescriptor, ResourceError, ResourceProvider};
use std::path::PathBuf;

/// One resource per `*.md` file in the active project's memory directory.
///
/// URIs: `memory://<stem>` where `<stem>` is the filename without extension.
pub struct MemoryProvider {
    dir: PathBuf,
}

impl MemoryProvider {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn entries(&self) -> Vec<(String, PathBuf)> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return out;
        };
        for e in rd.flatten() {
            // `DirEntry::file_type` does not follow a link: a leaf link, a dangling
            // link and a directory named `x.md` are not memory topics, and listing
            // them would advertise a resource that `read` refuses (it once served the
            // link's target).
            if !e.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    out.push((stem.to_string(), p));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// Resolve `memory://<stem>` to a `.md` path under `self.dir`.
    ///
    /// SAFETY / invariant: resolution is by **allowlist lookup** against the
    /// set enumerated by `self.entries()` (which only yields files produced by
    /// `read_dir(self.dir)` with `.md` extension). A URI like
    /// `memory://../../etc/passwd` simply misses the allowlist and returns
    /// `None` — there is no `self.dir.join(stem)` path construction.
    ///
    /// Do NOT "optimize" this to `self.dir.join(format!("{stem}.md"))` — that
    /// would re-introduce path traversal. If you need faster lookup, build a
    /// `HashMap<String, PathBuf>` from `entries()` once and index into it.
    fn lookup(&self, uri: &str) -> Option<PathBuf> {
        let stem = uri.strip_prefix("memory://")?;
        self.entries()
            .into_iter()
            .find(|(s, _)| s == stem)
            .map(|(_, p)| p)
    }

    /// Read an allowlisted entry.
    ///
    /// The allowlist was built without following links, but an entry can be swapped
    /// between that walk and this read, so the file type is checked again, as
    /// `MemoryStore::read` does. A stem is NOT handed to `MemoryStore::read`:
    /// `a.b.md` has stem `a.b`, which `topic_path` would map to `a.md`.
    async fn read_path_checked(
        &self,
        uri: &str,
        path: PathBuf,
    ) -> Result<ResourceBytes, ResourceError> {
        match tokio::fs::symlink_metadata(&path).await {
            Ok(m) if m.file_type().is_file() => {}
            Ok(_) => {
                return Err(ResourceError::SourceUnavailable(
                    uri.into(),
                    format!("memory target must be a regular file: {}", path.display()),
                ))
            }
            Err(e) => return Err(ResourceError::SourceUnavailable(uri.into(), e.to_string())),
        }
        let body = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ResourceError::SourceUnavailable(uri.into(), e.to_string()))?;
        Ok(ResourceBytes::Text(body))
    }
}

#[async_trait::async_trait]
impl ResourceProvider for MemoryProvider {
    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        self.entries()
            .into_iter()
            .map(|(stem, _)| ResourceDescriptor {
                uri: format!("memory://{}", stem),
                name: stem.clone(),
                description: Some(format!("Project memory: {}", stem)),
                mime_type: "text/markdown".into(),
            })
            .collect()
    }

    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        let path = self
            .lookup(uri)
            .ok_or_else(|| ResourceError::NotFound(uri.into()))?;
        self.read_path_checked(uri, path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_provider_enumerates_md_files_in_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("arch.md"), "arch body").unwrap();
        std::fs::write(tmp.path().join("NOT_MEMORY.txt"), "ignore").unwrap();
        let p = MemoryProvider::new(tmp.path().to_path_buf());
        let uris: Vec<_> = p.descriptors().into_iter().map(|d| d.uri).collect();
        assert!(uris.contains(&"memory://arch".to_string()));
        assert_eq!(uris.len(), 1);
    }

    #[tokio::test]
    async fn memory_provider_reads_file_body() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("notes.md"), "hello memory").unwrap();
        let p = MemoryProvider::new(tmp.path().to_path_buf());
        let bytes = p.read("memory://notes").await.unwrap();
        match bytes {
            ResourceBytes::Text(s) => assert_eq!(s, "hello memory"),
            _ => panic!("expected text"),
        }
    }

    /// `resources/list` and `resources/read` through the registry, the layer the MCP
    /// handler delegates to. A topic that is a link to a secret was both listed and
    /// served (`resources/read memory://arch` returned the target); a dangling link and
    /// a directory named `x.md` were listed as resources nothing could read.
    #[cfg(unix)]
    #[tokio::test]
    async fn the_registry_neither_lists_nor_serves_a_linked_or_non_file_topic() {
        use crate::mcp_resources::ResourceRegistry;
        let sandbox = tempfile::tempdir().unwrap();
        let secret = sandbox.path().join("home/.ssh/id_rsa");
        std::fs::create_dir_all(secret.parent().unwrap()).unwrap();
        std::fs::write(&secret, "SECRET-MARKER").unwrap();
        let dir = sandbox.path().join("memories");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("ok.md"), "fine").unwrap();
        std::os::unix::fs::symlink(&secret, dir.join("arch.md")).unwrap();
        std::os::unix::fs::symlink(sandbox.path().join("nowhere"), dir.join("ghost.md")).unwrap();
        std::fs::create_dir(dir.join("folder.md")).unwrap();

        let mut rr = ResourceRegistry::new();
        rr.try_register(Box::new(MemoryProvider::new(dir))).unwrap();

        let uris: Vec<String> = rr.list().into_iter().map(|d| d.uri).collect();
        assert_eq!(uris, vec!["memory://ok".to_string()]);
        for uri in ["memory://arch", "memory://ghost", "memory://folder"] {
            match rr.read(uri).await {
                Err(_) => {}
                Ok(ResourceBytes::Text(t)) => panic!("{uri} was served: {t:?}"),
                Ok(_) => panic!("{uri} was served"),
            }
        }
    }

    /// The allowlist is built without following links, so the read must re-check: an
    /// entry swapped for a link between `descriptors` and `read` is refused, not served.
    #[cfg(unix)]
    #[tokio::test]
    async fn read_refuses_an_entry_swapped_for_a_link_after_it_was_listed() {
        let sandbox = tempfile::tempdir().unwrap();
        let secret = sandbox.path().join("secret");
        std::fs::write(&secret, "SECRET-MARKER").unwrap();
        let dir = sandbox.path().join("memories");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("arch.md"), "fine").unwrap();
        let p = MemoryProvider::new(dir.clone());
        assert!(p.lookup("memory://arch").is_some());
        std::fs::remove_file(dir.join("arch.md")).unwrap();
        std::os::unix::fs::symlink(&secret, dir.join("arch.md")).unwrap();
        // `lookup` rebuilds the allowlist, so exercise the read-time check directly.
        let err = p
            .read_path_checked("memory://arch", dir.join("arch.md"))
            .await;
        assert!(
            matches!(err, Err(ResourceError::SourceUnavailable(..))),
            "got {err:?}"
        );
    }
}
