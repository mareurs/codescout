use super::{ResourceBytes, ResourceDescriptor, ResourceError, ResourceProvider};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct DocSource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub path: PathBuf,
}

pub struct DocProvider {
    sources: Vec<DocSource>,
}

impl DocProvider {
    pub fn new(sources: Vec<DocSource>) -> Self {
        Self { sources }
    }
}

#[async_trait::async_trait]
impl ResourceProvider for DocProvider {
    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        self.sources
            .iter()
            .map(|s| ResourceDescriptor {
                uri: s.uri.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                mime_type: "text/markdown".into(),
            })
            .collect()
    }

    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        let src = self
            .sources
            .iter()
            .find(|s| s.uri == uri)
            .ok_or_else(|| ResourceError::NotFound(uri.into()))?;
        let body = tokio::fs::read_to_string(&src.path)
            .await
            .map_err(|e| ResourceError::SourceUnavailable(uri.into(), e.to_string()))?;
        Ok(ResourceBytes::Text(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn doc_provider_reads_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("guide.md");
        std::fs::write(&path, "# hello").unwrap();
        let p = DocProvider::new(vec![DocSource {
            uri: "doc://guide".into(),
            name: "guide".into(),
            description: None,
            path,
        }]);
        let bytes = p.read("doc://guide").await.unwrap();
        match bytes {
            ResourceBytes::Text(s) => assert_eq!(s, "# hello"),
            _ => panic!("expected text"),
        }
    }

    #[tokio::test]
    async fn doc_provider_reports_missing_source() {
        let p = DocProvider::new(vec![DocSource {
            uri: "doc://missing".into(),
            name: "missing".into(),
            description: None,
            path: PathBuf::from("/nonexistent/path/does/not/exist"),
        }]);
        let err = p.read("doc://missing").await.unwrap_err();
        assert!(matches!(err, ResourceError::SourceUnavailable(_, _)));
    }
}
