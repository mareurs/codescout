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
        let body = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ResourceError::SourceUnavailable(uri.into(), e.to_string()))?;
        Ok(ResourceBytes::Text(body))
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
}
